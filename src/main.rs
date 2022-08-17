use chrono::{NaiveDateTime, TimeZone, Utc};
use num_format::{Locale, ToFormattedString};
use std::fmt::Write;

use nostr_bot::{
    log::debug, tokio, unix_timestamp, wrap, Command, Event, EventNonSigned, FunctorType,
};

mod mempool;

struct Info {
    last_block_hash: String,
    start_timestamp: u64,
}

type State = nostr_bot::State<Info>;

fn format(value: &serde_json::Value) -> String {
    let num = value.to_string().parse::<u64>().unwrap();
    num.to_formatted_string(&Locale::en)
}

async fn get_new_blocks(
    last_block_hash: String,
) -> Result<(String, Vec<serde_json::Value>), reqwest::Error> {
    let current_block_hash = mempool::block_tip_hash().await?;

    debug!(
        "last_block_hash: {}, current_block_hash: {}",
        last_block_hash, current_block_hash
    );
    let mut block_hash = current_block_hash.clone();

    let mut blocks = vec![];
    while block_hash != last_block_hash {
        let block: serde_json::Value =
            serde_json::from_str(&mempool::get_block(&block_hash).await?).unwrap();
        block_hash = block["previousblockhash"].to_string().replace('\"', "");
        blocks.push(block);
    }

    Ok((current_block_hash, blocks))
}

async fn uptime(event: Event, state: State) -> EventNonSigned {
    let start_timestamp = state.lock().await.start_timestamp;
    let timestamp = unix_timestamp();

    let running_secs = timestamp - start_timestamp;

    nostr_bot::get_reply(
        event,
        format!(
            "Running for {}",
            compound_duration::format_dhms(running_secs)
        ),
    )
}

fn format_blocks(blocks: Vec<serde_json::Value>) -> EventNonSigned {
    let mut content = format!("Got {} newly mined block(s):\n", blocks.len());
    let mut tags = vec![vec!["#t".to_string(), "bitcoin".to_string()]];

    for (i, block) in blocks.iter().enumerate() {
        writeln!(content, "{}", block["id"].to_string().replace('\"', "")).unwrap();
        writeln!(content, "- height: {}", format(&block["height"])).unwrap();

        let timestamp = block["timestamp"].to_string().parse::<i64>().unwrap();
        let timestamp = Utc
            .from_local_datetime(&NaiveDateTime::from_timestamp(timestamp, 0))
            .unwrap();
        writeln!(content, "- timestamp: {}", timestamp).unwrap();

        writeln!(content, "- tx count: {}", format(&block["tx_count"])).unwrap();
        writeln!(content, "- size: {}", format(&block["size"])).unwrap();
        writeln!(content, "- weight: {}", format(&block["weight"])).unwrap();

        let block_url = format!(
            "https://mempool.space/block/{}",
            block["id"].to_string().replace('"', "")
        );
        writeln!(content, "- {}", &block_url).unwrap();
        tags.push(vec!["#r".to_string(), block_url]);

        if i + 1 < blocks.len() {
            writeln!(content).unwrap();
        }
    }

    EventNonSigned {
        created_at: unix_timestamp(),
        kind: 1,
        content,
        tags,
    }
}

#[tokio::main]
async fn main() {
    nostr_bot::init_logger();

    let mut secret = std::fs::read_to_string("secret").unwrap();
    secret.pop(); // Remove newline
    let keypair = nostr_bot::keypair_from_secret(&secret);

    let relays = vec![
        "wss://nostr-pub.wellorder.net",
        "wss://relay.nostr.info",
        "wss://relay.damus.io",
        "wss://nostr.delo.software",
        "wss://nostr.zaprite.io",
    ];

    let args = std::env::args().collect::<Vec<_>>();

    let last_block_hash = if args.len() == 2 {
        nostr_bot::log::info!("Using {} as last block.", args[1]);
        args[1].to_string()
    } else {
        nostr_bot::log::warn!("Last block hash not specified, using current tip.");
        mempool::block_tip_hash().await.unwrap()
    };

    let state = nostr_bot::wrap_state(Info {
        last_block_hash,
        start_timestamp: unix_timestamp(),
    });

    let sender = nostr_bot::new_sender();

    // TODO: Cleanup
    let update = {
        let sender = sender.clone();
        let state = state.clone();
        async move {
            let errors_discard_period = std::time::Duration::from_secs(3600);
            let mut last_error_time = std::time::SystemTime::now();

            loop {
                let last_block_hash = state.lock().await.last_block_hash.clone();

                match get_new_blocks(last_block_hash).await {
                    Ok((new_block_tip, new_blocks)) => {
                        state.lock().await.last_block_hash = new_block_tip;
                        if !new_blocks.is_empty() {
                            let event = format_blocks(new_blocks);
                            sender.lock().await.send(event.sign(&keypair)).await;
                        }
                    }
                    Err(_e) => {
                        let now = std::time::SystemTime::now();
                        if now.duration_since(last_error_time).unwrap() > errors_discard_period {
                            let event = EventNonSigned {
                                created_at: unix_timestamp(),
                                kind: 1,
                                content: String::from("I'm unable to reach the API."),
                                tags: vec![],
                            };
                            sender.lock().await.send(event.sign(&keypair)).await;
                            last_error_time = now;
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        }
    };

    nostr_bot::Bot::new(keypair, relays, state)
        .name("bitcoin_bot")
        .about("Bot publishing info about newly mined blocks. Using https://mempool.space/ API.")
        .picture("https://upload.wikimedia.org/wikipedia/commons/5/50/Bitcoin.png")
        .intro_message("Hi, I will be posting info about newly mined blocks.")
        .command(
            Command::new("!uptime", wrap!(uptime))
                .description("Show for how long is the bot running."),
        )
        .sender(sender)
        .spawn(Box::pin(update))
        .help()
        .run()
        .await;
}
