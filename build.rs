use std::{
    collections::HashMap,
    ffi::OsStr,
    fmt::Write as _,
    fs::{self, File},
    io::Write as _,
};

#[derive(Clone, Debug)]
struct Tag {
    id: usize,
    name: &'static str,
    desc: Option<&'static str>,
    sub: Vec<Tag>,
}

fn main() {
    let _ = fs::create_dir("frontend/modules/src/bindings");
    let exports: Vec<_> = fs::read_dir("frontend/modules/src/bindings")
        .expect("read dir")
        .filter_map(Result::ok)
        .filter_map(|p| {
            p.path()
                .file_stem()
                .map(OsStr::to_str)
                .flatten()
                .map(str::to_owned)
        })
        .filter(|f| f != "index")
        .map(|f| format!("export * from \"./{}\"", f))
        .collect();

    let mut file = File::create("frontend/modules/src/bindings/index.ts").unwrap();
    file.write_all(exports.join("\n").as_bytes()).unwrap();

    fn walk(
        tags: &[Tag],
        mut parents: Vec<Tag>,
        implications: &mut HashMap<usize, Vec<Tag>>,
        flat_tags: &mut HashMap<usize, (String, Tag)>,
    ) {
        for tag in tags {
            let mut path = parents.iter().map(|parent| parent.name).collect::<Vec<_>>();
            path.push(tag.name);
            let path = path.join("/");
            for parent in parents.iter() {
                implications.entry(parent.id).or_default().push(tag.clone());
            }
            assert!(
                flat_tags.insert(tag.id, (path, tag.clone())).is_none(),
                "tag ID already in use"
            );

            parents.push(tag.clone());
            //println!("{parents:?} {}", tag.name);
            walk(&tag.sub, parents.clone(), implications, flat_tags);
            parents.pop();
        }
    }

    let mut implications = HashMap::new();
    let mut flat_tags = HashMap::new();

    walk(&tags(), vec![], &mut implications, &mut flat_tags);

    let mut migration = String::new();

    let mut implications_list = implications.iter().collect::<Vec<_>>();
    implications_list.sort_by_key(|(parent, _)| *parent);
    let implications_enumerated = implications_list.iter().enumerate().collect::<Vec<_>>();
    for (implication, (implied, implyers)) in implications_enumerated.iter() {
        for implyer in implyers.iter() {
            writeln!(
                &mut migration,
                "INSERT INTO tag_implies (implication, implyer, implied) VALUES ({}, {}, {});",
                implication, implyer.id, implied
            )
            .unwrap();
        }
    }

    let mut flat_tags = flat_tags.into_values().collect::<Vec<_>>();
    flat_tags.sort_by_key(|(_, tag)| tag.id);
    for (path, tag) in flat_tags {
        if let Some((implication, _)) = implications_enumerated.iter().find(|(_, (_, implyers))| {
            implyers
                .iter()
                .any(|implying_tag| implying_tag.id == tag.id)
        }) {
            writeln!(migration,
                "INSERT INTO tag (tag_id, tag_name, tag_definition, implication) VALUES ({}, '{}', '{}', {});",
                tag.id, path, tag.desc.unwrap_or_default(), implication).unwrap();
        } else {
            writeln!(
                migration,
                "INSERT INTO tag (tag_id, tag_name, tag_definition) VALUES ({}, '{}', '{}');",
                tag.id,
                path,
                tag.desc.unwrap_or_default()
            )
            .unwrap();
        }
    }

    // accel_pen=# SELECT b.tag_id, b.tag_name FROM tag AS a JOIN tag_implies ON tag_implies.implyer = a.tag_id JOIN tag AS b ON b.tag_id = tag_implies.implied WHERE a.tag_id = 26;
    // accel_pen=# SELECT b.tag_id, b.tag_name FROM tag AS a JOIN tag_implies ON tag_implies.implied = a.tag_id JOIN tag AS b ON b.tag_id = tag_implies.implyer WHERE a.tag_id = 21;

    std::fs::write("migrations/0002_tags.up.sql", migration).unwrap();
}

fn tags() -> Vec<Tag> {
    vec![
        Tag {
            id: 1,
            name: "Style",
            desc: None,
            sub: vec![
                Tag {
                    id: 2,
                    name: "FullSpeed",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 3,
                    name: "SpeedFun",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 4,
                    name: "Tech",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 5,
                    name: "SpeedTech",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 6,
                    name: "RPG",
                    desc: None,
                    sub: vec![Tag {
                        id: 7,
                        name: "Short",
                        desc: None,
                        sub: vec![],
                    }],
                },
                Tag {
                    id: 8,
                    name: "MiniRPG",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 9,
                    name: "LOL",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 10,
                    name: "Trial",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 11,
                    name: "ZrT",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 12,
                    name: "Competitive",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 13,
                    name: "Kacky",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 14,
                    name: "Endurance",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 15,
                    name: "Obstacle",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 16,
                    name: "Nascar",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 17,
                    name: "Transitional",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 18,
                    name: "Backwards",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 19,
                    name: "Pathfinding",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 20,
                    name: "Underwater",
                    desc: None,
                    sub: vec![],
                },
            ],
        },
        Tag {
            id: 21,
            name: "Surface",
            desc: None,
            sub: vec![
                Tag {
                    id: 22,
                    name: "Mixed",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 23,
                    name: "Offroad",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 24,
                    name: "Turtle",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 25,
                    name: "Ice",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 26,
                    name: "Bobsleigh",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 27,
                    name: "Dirt",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 28,
                    name: "Plastic",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 29,
                    name: "Grass",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 30,
                    name: "Wood",
                    desc: None,
                    sub: vec![
                        Tag {
                            id: 31,
                            name: "WetWood",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 32,
                            name: "WetIcyWood",
                            desc: None,
                            sub: vec![],
                        },
                    ],
                },
                Tag {
                    id: 33,
                    name: "Water",
                    desc: None,
                    sub: vec![],
                },
            ],
        },
        Tag {
            id: 34,
            name: "Feature",
            desc: None,
            sub: vec![
                Tag {
                    id: 35,
                    name: "Gear",
                    desc: None,
                    sub: vec![
                        Tag {
                            id: 36,
                            name: "2",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 37,
                            name: "3",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 38,
                            name: "4",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 39,
                            name: "5",
                            desc: None,
                            sub: vec![],
                        },
                    ],
                },
                Tag {
                    id: 40,
                    name: "TireStatus",
                    desc: None,
                    sub: vec![
                        Tag {
                            id: 41,
                            name: "Worn",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 42,
                            name: "Wet",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 43,
                            name: "Icy",
                            desc: None,
                            sub: vec![],
                        },
                    ],
                },
                Tag {
                    id: 44,
                    name: "NoBrakes",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 45,
                    name: "Reactor",
                    desc: None,
                    sub: vec![Tag {
                        id: 46,
                        name: "YEET",
                        desc: None,
                        sub: vec![],
                    }],
                },
                Tag {
                    id: 47,
                    name: "SlowMotion",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 48,
                    name: "Fragile",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 49,
                    name: "EngineOff",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 50,
                    name: "CruiseControl",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 51,
                    name: "NoSteering",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 52,
                    name: "NoGrip",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 53,
                    name: "Sausage",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 54,
                    name: "Magnet",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 55,
                    name: "Bumpers",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 56,
                    name: "MovingItems",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 57,
                    name: "Pipes",
                    desc: None,
                    sub: vec![],
                },
            ],
        },
        Tag {
            id: 58,
            name: "OriginalCar",
            desc: None,
            sub: vec![
                Tag {
                    id: 59,
                    name: "Snow",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 60,
                    name: "Desert",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 61,
                    name: "Rally",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 62,
                    name: "Mixed",
                    desc: None,
                    sub: vec![],
                },
            ],
        },
        Tag {
            id: 63,
            name: "Meta",
            desc: None,
            sub: vec![
                Tag {
                    id: 64,
                    name: "AlteredNadeo",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 65,
                    name: "Mini",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 66,
                    name: "SecretLeaderboard",
                    desc: None,
                    sub: vec![Tag {
                        id: 67,
                        name: "WeeklyShorts",
                        desc: None,
                        sub: vec![],
                    }],
                },
                Tag {
                    id: 68,
                    name: "Scenery",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 69,
                    name: "Signature",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 70,
                    name: "Educational",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 71,
                    name: "SpeedMapping",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 72,
                    name: "PressForward",
                    desc: None,
                    sub: vec![],
                },
                Tag {
                    id: 73,
                    name: "Gamemode",
                    desc: None,
                    sub: vec![
                        Tag {
                            id: 74,
                            name: "Stunt",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 75,
                            name: "Platform",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 76,
                            name: "Royal",
                            desc: None,
                            sub: vec![],
                        },
                        Tag {
                            id: 77,
                            name: "Puzzle",
                            desc: None,
                            sub: vec![],
                        },
                    ],
                },
                Tag {
                    id: 78,
                    name: "Experimental",
                    desc: None,
                    sub: vec![],
                },
            ],
        },
    ]
}
