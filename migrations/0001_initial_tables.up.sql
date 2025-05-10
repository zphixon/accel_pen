CREATE TABLE ap_user (
    ap_user_id SERIAL UNIQUE NOT NULL,
    nadeo_display_name TEXT NOT NULL,
    nadeo_id TEXT UNIQUE NOT NULL,
    nadeo_login TEXT UNIQUE NOT NULL,
    site_admin BOOLEAN NOT NULL DEFAULT FALSE,
    registered TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', now())),
    nadeo_club_tag TEXT,
    deleted TIMESTAMP WITH TIME ZONE,

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
    deleted TIMESTAMP WITH TIME ZONE,

    CONSTRAINT pk_map PRIMARY KEY (ap_map_id, gbx_mapuid),
    CONSTRAINT fk_map_author FOREIGN KEY (ap_author_id)
        REFERENCES ap_user (ap_user_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE tag_name (
    tag_id SERIAL UNIQUE NOT NULL,
    tag_name TEXT NOT NULL,
    tag_kind TEXT NOT NULL,
    tag_definition TEXT,

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

INSERT INTO tag_name (tag_name, tag_kind) VALUES
    ('FullSpeed',         'mapStyle'),
    ('SpeedFun',          'mapStyle'),
    ('Tech',              'mapStyle'),
    ('SpeedTech',         'mapStyle'),
    ('RPG',               'mapStyle'),
    ('MiniRPG',           'mapStyle'),
    ('LOL',               'mapStyle'),
    ('Trial',             'mapStyle'),
    ('ZrT',               'mapStyle'),
    ('Competitive',       'mapStyle'),
    ('Kacky',             'mapStyle'),
    ('Endurance',         'mapStyle'),
    ('Obstacle',          'mapStyle'),
    ('Mixed',             'mapStyle'),
    ('Nascar',            'mapStyle'),
    ('Transitional',      'mapStyle'),
    ('Backwards',         'mapStyle'),
    ('Pathfinding',       'mapStyle'),

    ('Offroad',           'definingSurface'),
    ('Underwater',        'definingSurface'),
    ('Turtle',            'definingSurface'),
    ('Ice',               'definingSurface'),
    ('Bobsleigh',         'definingSurface'),
    ('WetWood',           'definingSurface'),
    ('WetIcyWood',        'definingSurface'),
    ('Dirt',              'definingSurface'),
    ('Plastic',           'definingSurface'),
    ('Grass',             'definingSurface'),
    ('Wood',              'definingSurface'),

    ('NoBrakes',          'mapFeature'),
    ('Reactor',           'mapFeature'),
    ('SlowMotion',        'mapFeature'),
    ('Fragile',           'mapFeature'),
    ('EngineOff',         'mapFeature'),
    ('CruiseControl',     'mapFeature'),
    ('NoSteering',        'mapFeature'),
    ('NoGrip',            'mapFeature'),
    ('Water',             'mapFeature'),
    ('Sausage',           'mapFeature'),
    ('Magnet',            'mapFeature'),
    ('Bumper',            'mapFeature'),
    ('MovingItems',       'mapFeature'),
    ('Pipes',             'mapFeature'),

    ('Bugslide',          'drivingTechnique'),
    ('Mudslide',          'drivingTechnique'),
    ('Gear3',             'drivingTechnique'),
    ('Gear4',             'drivingTechnique'),

    ('SnowCar',           'notStadium'),
    ('DesertCar',         'notStadium'),
    ('RallyCar',          'notStadium'),
    ('MixedCar',          'notStadium'),

    ('AlteredNadeo',      'mapMeta'),
    ('Mini',              'mapMeta'),
    ('SecretLeaderboard', 'mapMeta'),
    ('Scenery',           'mapMeta'),
    ('Signature',         'mapMeta'),
    ('Educational',       'mapMeta'),
    ('SpeedMapping',      'mapMeta'),
    ('PressForward',      'mapMeta'),
    ('Race',              'mapMeta'),
    ('Stunt',             'mapMeta'),
    ('Platform',          'mapMeta'),
    ('Royal',             'mapMeta'),
    ('Puzzle',            'mapMeta')
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
