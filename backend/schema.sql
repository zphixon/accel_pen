CREATE TABLE user (
    user_id INTEGER UNSIGNED AUTO_INCREMENT,
    display_name VARCHAR(128) NOT NULL,
    account_id CHAR(36) UNIQUE NOT NULL,
    site_admin BOOLEAN NOT NULL DEFAULT FALSE,
    registered TIMESTAMP NOT NULL,

    CONSTRAINT pk_user PRIMARY KEY (user_id)
);

CREATE TABLE map (
    ap_id INTEGER UNSIGNED UNIQUE AUTO_INCREMENT,
    gbx_mapuid VARCHAR(128) UNIQUE NOT NULL,
    gbx_data MEDIUMBLOB NOT NULL,
    mapname TEXT NOT NULL,
    author INTEGER UNSIGNED NOT NULL,
    votes INTEGER NOT NULL DEFAULT 1,
    uploaded TIMESTAMP NOT NULL,
    created TIMESTAMP NOT NULL,

    CONSTRAINT pk_map PRIMARY KEY (ap_id, gbx_mapuid),
    CONSTRAINT fk_map_author FOREIGN KEY (author)
        REFERENCES user (user_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE vote (
    user_id INTEGER UNSIGNED,
    ap_id INTEGER UNSIGNED,
    vote_value TINYINT NOT NULL,
    cast TIMESTAMP NOT NULL,

    CONSTRAINT pk_vote PRIMARY KEY (user_id, ap_id),
    CONSTRAINT fk_vote_user FOREIGN KEY (user_id)
        REFERENCES user (user_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT fk_vote_map FOREIGN KEY (ap_id)
        REFERENCES map (ap_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);
