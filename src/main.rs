use chrono::{NaiveDateTime, TimeZone, Utc};
use num_format::{Locale, ToFormattedString};
use std::fmt::Write;

use nostr_bot::{log::debug, tokio, unix_timestamp, EventNonSigned};

mod mempool;

struct Info {
    last_block_hash: String,
}

// type State = nostr_bot::State<Info>;

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

fn format_blocks(blocks: Vec<serde_json::Value>) -> EventNonSigned {
    let mut content = format!("Got {} newly mined block(s):\n", blocks.len());

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
        writeln!(content, "- https://mempool.space/block/{}", block["id"].to_string().replace('"', "")).unwrap();

        if i + 1 < blocks.len() {
            writeln!(content, "").unwrap();
        }

    }

    EventNonSigned {
        created_at: unix_timestamp(),
        kind: 1,
        content,
        tags: vec![],
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
        "wss://relay.damus.io",
        "wss://relay.nostr.info",
    ];

    let current_tip_hash = mempool::block_tip_hash().await.unwrap();
    let state = nostr_bot::wrap_state(Info {
        last_block_hash: current_tip_hash,
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
                                created_at: nostr_bot::unix_timestamp(),
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
        // .command(Command::new("!difficulty", wrap!(difficulty)))
        .sender(sender)
        .spawn(Box::pin(update))
        .help()
        .run()
        .await;
}
