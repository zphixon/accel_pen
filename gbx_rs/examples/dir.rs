use std::io::Read;

fn main() -> Result<(), &'static str> {
    tracing_subscriber::fmt::init();
    tracing::info!("chugnus");

    let args = std::env::args().collect::<Vec<_>>();
    let [_, dirname] = &args[..] else {
        return Err("expected one arg, map dir name");
    };

    let mut queue = vec![std::path::PathBuf::from(dirname.clone())];
    while let Some(next) = queue.pop() {
        if std::fs::metadata(&next).expect("metadata of file").is_dir() {
            for file in std::fs::read_dir(&next).expect("read dir") {
                let file = file.expect("read dir entry");
                queue.push(file.path());
            }
        } else {
            println!("{}", next.display());
            let mut file = std::fs::File::open(next).map_err(|_| "couldn't open file")?;
            let mut data = Vec::new();
            file.read_to_end(&mut data)
                .map_err(|_| "couldn't read file")?;

            if let Ok(node) = gbx_rs::Node::read_from(&data) {
                let Ok(gbx_rs::parse::CGame::CtnChallenge(map)) = node.parse() else {
                    println!("    not a map");
                    continue;
                };
                println!(
                    "    {} by {}",
                    map.map_name.unwrap(),
                    map.map_info.unwrap().author
                );
                println!(
                    "    AT {}, glod {}",
                    map.challenge_parameters
                        .as_ref()
                        .unwrap()
                        .author_time
                        .unwrap(),
                    map.challenge_parameters
                        .as_ref()
                        .unwrap()
                        .gold_time
                        .unwrap(),
                );

                if let Some(thumbnail) = map.thumbnail_data {
                    println!("thumbnail format: {:?}", image::guess_format(thumbnail));
                }
            } else {
                println!("    not a map hmmm")
            }
        }
    }

    Ok(())
}
