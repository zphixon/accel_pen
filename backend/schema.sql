CREATE TABLE user (
    user_id INTEGER UNSIGNED AUTO_INCREMENT,
    display_name VARCHAR(128) UNIQUE NOT NULL,
    nadeo_login VARCHAR(128) UNIQUE NOT NULL,
    ubi_account_id CHAR(36) UNIQUE NOT NULL,
    site_admin BOOLEAN NOT NULL DEFAULT FALSE,

    PRIMARY KEY (user_id)
);

CREATE TABLE map (
    ap_id INTEGER UNSIGNED AUTO_INCREMENT,
    gbx_mapuid VARCHAR(128) NOT NULL,
    gbx_data BLOB NOT NULL,
    mapname TEXT NOT NULL,
    author INTEGER UNSIGNED NOT NULL,

    PRIMARY KEY (ap_id),
    FOREIGN KEY (author)
        REFERENCES user (user_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);
