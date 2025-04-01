use tokio::io::BufReader;

#[tokio::main]
async fn main() -> Result<(), ()> {
    tracing_subscriber::fmt::init();
    tracing::info!("chugnus");

    let args = std::env::args().collect::<Vec<_>>();
    let [_, filename] = &args[..] else {
        tracing::error!("expected one arg, map filename");
        return Err(());
    };

    let file = tokio::fs::File::open(filename).await.map_err(|_| {})?;
    let reader = BufReader::new(file);

    let header = gbx_rs::parse_headers(reader).await.map_err(|err| {
        println!("{:?}", err);
    })?;
    println!("{:#?}", header);

    Ok(())
}