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
            //match node.to::<CGameCtnChallenge>() {
            //    Ok(mut map) => match map.read_full() {
            //        Ok(()) => {
            //            println!("{:?}", map.map_name.unwrap());
            //            println!(
            //                "{:?}",
            //                map.challenge_parameters.unwrap().author_time.unwrap()
            //            );
            //        }
            //        Err(err) => {
            //            println!("couldn't read map in full:\n{}", err);
            //        }
            //    },
            //    Err(err) => {
            //        println!("couldn't read as a map:\n{}", err);
            //    }
            //}

            match node.parse() {
                Ok(gbx_rs::parse::CGame::CtnChallenge(map)) => {
                    println!("got a map: {:#?}", map);
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
