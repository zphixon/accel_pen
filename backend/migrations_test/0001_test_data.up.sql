INSERT INTO user (
    display_name,
    nadeo_login,
    ubi_account_id,
    site_admin,
    registered
) VALUES (
    'cheezgi',
    'nadeo-random',
    'ubi-random',
    1,
    NOW()
);

INSERT INTO map (
    gbx_mapuid,
    gbx_data,
    mapname,
    author,
    uploaded,
    created
) VALUES (
    'gbx mapuid',
    'gbx data',
    'Borasisi',
    (SELECT user_id FROM user WHERE display_name = 'cheezgi' LIMIT 1),
    NOW(),
    NOW()
);
