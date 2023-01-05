//! A simple CLI app that downloads the contents at a URL and saves it into
//! a file.

use quickload_loader::ByteSize;

const CHUNK_SIZE: ByteSize = 4 * 1024 * 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    quickload_disk_space_allocation::prepare_privileges()?;

    let mut args = std::env::args().skip(1);
    let url = args.next().ok_or("pass url as a first argument")?;
    let file_path = args.next().ok_or("pass file path as a second argument")?;

    let https = hyper_tls::HttpsConnector::new();
    let client = hyper::Client::builder().build::<_, hyper::Body>(https);
    let url = url.parse()?;
    let total_size = quickload_loader::Loader::detect_size(&client, &url).await?;
    let loader = quickload_loader::Loader::with_size(
        file_path,
        client,
        url,
        total_size,
        CHUNK_SIZE.try_into().unwrap(),
    )?;
    loader.run().await?;

    Ok(())
}
