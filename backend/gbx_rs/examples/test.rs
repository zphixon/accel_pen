use std::io::Read;

use gbx_rs::CGameCtnChallenge;

fn main() -> Result<(), &'static str> {
    tracing_subscriber::fmt::init();
    tracing::info!("chugnus");

    let args = std::env::args().collect::<Vec<_>>();
    let [_, filename] = &args[..] else {
        return Err("expected one arg, map filename");
    };

    let mut file = std::fs::File::open(filename).map_err(|_| "couldn't open file")?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|_| "couldn't read file")?;
    let mut node = gbx_rs::Node::read_from(std::io::Cursor::new(data)).map_err(|err| {
        println!("{:?}", err);
        "couldn't parse file"
    })?;
    println!("{:#?}", node);

    println!("{:?}", node.to::<CGameCtnChallenge<_>>().unwrap().map_name());

    Ok(())
}
