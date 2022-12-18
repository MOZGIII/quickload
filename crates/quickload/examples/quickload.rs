//! A simple CLI app that downloads the contents at a URL and saves it into
//! a file.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    quickload::disk_space_allocation::prepare_privileges()?;

    let mut args = std::env::args().skip(1);
    let url = args.next().ok_or("pass url as a first argument")?;
    let file_path = args.next().ok_or("pass file path as a second argument")?;

    let https = hyper_tls::HttpsConnector::new();
    let client = hyper::Client::builder().build::<_, hyper::Body>(https);
    let url = url.parse()?;
    let loader = quickload::Loader::detect_size(file_path, client, url).await?;
    loader.run().await?;

    Ok(())
}
