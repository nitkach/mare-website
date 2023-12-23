create table if not exists mares (
             id serial       primary key,
           name varchar(100) not null,
          breed integer      not null,
    modified_at timestamptz  not null     default (now()::timestamp)
);
