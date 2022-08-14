const BASE: &str = "https://mempool.space/api";

async fn get_response(url: &str) -> Result<String, reqwest::Error> {
    reqwest::get(url).await?.text().await
}

pub async fn block_tip_hash() -> Result<String, reqwest::Error> {
    let url = format!("{}/{}", BASE, "blocks/tip/hash");

    get_response(&url).await
}

pub async fn get_block(hash: &str) -> Result<String, reqwest::Error> {
    let url = format!("{}/block/{}", BASE, hash);
    get_response(&url).await
}
