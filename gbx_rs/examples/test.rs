use std::io::Read;

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

    match gbx_rs::Node::read_from(&data) {
        Ok(node) => {
            println!("{:#?}", node);
            match node.parse() {
                Ok(gbx_rs::parse::CGame::CtnChallenge(map)) => {
                    println!("got a map: {:#?}", map);
                    if let Some(thumbnail) = map.thumbnail_data {
                        //let image = image::ImageReader::new(std::io::Cursor::new(thumbnail)).with_guessed_format().unwrap().decode().unwrap();
                        println!("thumbnail format: {:?}", image::guess_format(thumbnail));
                    }
                }
                Ok(_) => {
                    println!("didn't get a map??");
                }
                Err(err) => {
                    println!("{}", err);
                }
            }
        }

        Err(err) => println!("{}", err),
    }

    Ok(())
}
