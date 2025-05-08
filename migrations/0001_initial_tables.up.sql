CREATE TABLE ap_user (
    ap_user_id SERIAL UNIQUE NOT NULL,
    nadeo_display_name TEXT NOT NULL,
    nadeo_id TEXT UNIQUE NOT NULL,
    nadeo_login TEXT UNIQUE NOT NULL,
    site_admin BOOLEAN NOT NULL DEFAULT FALSE,
    registered TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', now())),
    nadeo_club_tag TEXT,

    CONSTRAINT pk_user PRIMARY KEY (ap_user_id)
);

CREATE TABLE map (
    ap_map_id SERIAL UNIQUE NOT NULL,
    gbx_mapuid TEXT UNIQUE NOT NULL,
    gbx_data BYTEA NOT NULL,
    mapname TEXT NOT NULL,
    ap_author_id INTEGER NOT NULL,
    votes INTEGER NOT NULL DEFAULT 1,
    uploaded TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', now())),
    created TIMESTAMP WITH TIME ZONE NOT NULL,
    thumbnail BYTEA NOT NULL,

    CONSTRAINT pk_map PRIMARY KEY (ap_map_id, gbx_mapuid),
    CONSTRAINT fk_map_author FOREIGN KEY (ap_author_id)
        REFERENCES ap_user (ap_user_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE tag_name (
    tag_id SERIAL UNIQUE NOT NULL,
    tag_name TEXT NOT NULL,

    CONSTRAINT pk_tag_name PRIMARY KEY (tag_id)
);

CREATE TABLE tag (
    ap_map_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,

    CONSTRAINT fk_map_id FOREIGN KEY (ap_map_id)
        REFERENCES map (ap_map_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT fk_tag_id FOREIGN KEY (tag_id)
        REFERENCES tag_name (tag_id)
);

INSERT INTO tag_name (tag_name) VALUES
    ('Race'),
    ('Scenery'),
    ('Signature'),
    ('Educational'),

    ('FullSpeed'),
    ('SpeedFun'),
    ('Tech'),
    ('SpeedTech'),
    ('RPG'),
    ('MiniRPG'),
    ('LOL'),
    ('Offroad'),
    ('Trial'),
    ('ZrT'),
    ('Competitive'),
    ('Kacky'),
    ('Endurance'),
    ('Obstacle'),
    ('Mixed'),
    ('Nascar'),
    ('Transitional'),
    ('Backwards'),
    ('Pathfinding'),
    ('AlteredNadeo'),
    ('Underwater'),
    ('Turtle'),

    ('NoBrakes'),
    ('Reactor'),
    ('SlowMotion'),
    ('Fragile'),
    ('EngineOff'),
    ('CruiseControl'),
    ('NoSteering'),
    ('NoGrip'),

    ('Ice'),
    ('Dirt'),
    ('Plastic'),
    ('Grass'),
    ('Water'),
    ('Sausage'),
    ('Bobsleigh'),
    ('Wood'),
    ('WetWood'),
    ('WetIcyWood'),
    ('Magnet'),
    ('Bumper'),
    ('MovingItems'),
    ('Pipes'),

    ('Bugslide'),
    ('Mudslide'),
    ('SpeedMapping'),

    ('SnowCar'),
    ('DesertCar'),
    ('RallyCar'),
    ('MixedCar'),

    ('PressForward'),
    ('Stunt'),
    ('Platform'),
    ('Royal'),
    ('Puzzle')
;

CREATE TABLE vote (
    ap_user_id INTEGER NOT NULL,
    ap_map_id INTEGER NOT NULL,
    vote_value SMALLINT NOT NULL,
    cast_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', now())),

    CONSTRAINT pk_vote PRIMARY KEY (ap_user_id, ap_map_id),
    CONSTRAINT fk_vote_user FOREIGN KEY (ap_user_id)
        REFERENCES ap_user (ap_user_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT fk_vote_map FOREIGN KEY (ap_map_id)
        REFERENCES map (ap_map_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);
