
pub(crate) const DEF_FILE_TEMPLATE : &str = r#"
<service>
  <id>{SERVICE_ID}</id>
  <name>{SERVICE_NAME}</name>
  <description>{SERVICE_NAME}</description>
  <executable>{EXECUTABLE}</executable>
  <logmode>rotate</logmode>
  <persistent_env name="DOKA_ENV" value="{MY_ENV}" />
</service>
"#;

pub(crate) const STD_APP_PROPERTIES_TEMPLATE: &str = r#"
#Rocket server port
server.port={SERVICE_PORT}
app.ek={SERVICE_CEK}

#database source
db.hostname={DB_HOST}
db.port={DB_PORT}
db.name=ad_{DOKA_INSTANCE}
db.user={DB_USER}
db.password={DB_PASSWORD}
db.pool_size=10

#Normalize log configuration path.
log4rs.config={SERVICE_LOG4RS}
"#;


pub(crate) const ADMIN_SERVER_APP_PROPERTIES_TEMPLATE: &str = r#"
#Rocket server port
server.port={SERVICE_PORT}
app.ek={SERVICE_CEK}

#database source
db.hostname={DB_HOST}
db.port={DB_PORT}
db.name=ad_{DOKA_INSTANCE}
db.user={DB_USER}
db.password={DB_PASSWORD}
db.pool_size=10

#cs db server
cs_db.hostname={DB_HOST}
cs_db.port={DB_PORT}
cs_db.name=cs_{DOKA_INSTANCE}
cs_db.user={DB_USER}
#cs_db.password=<same as above>

#fs db server
fs_db.hostname={DB_HOST}
fs_db.port={DB_PORT}
fs_db.name=fs_{DOKA_INSTANCE}
fs_db.user={DB_USER}
#cs_db.password=<same as above>

#Key Manager service
km.host={KM_HOST}
km.port={KM_PORT}
sm.host={SM_HOST}
sm.port={SM_PORT}

#Normalize log configuration path.
log4rs.config={SERVICE_LOG4RS}
"#;

pub(crate) const DOCUMENT_SERVER_APP_PROPERTIES_TEMPLATE: &str = r#"
#Rocket server port
server.port={SERVICE_PORT}
app.ek={SERVICE_CEK}

#database source
db.hostname={DB_HOST}
db.port={DB_PORT}
db.name=ad_{DOKA_INSTANCE}
db.user={DB_USER}
db.password={DB_PASSWORD}
db.pool_size=10

#Key Manager service
km.host={KM_HOST}
km.port={KM_PORT}
#Session Manager service
sm.host={SM_HOST}
sm.port={SM_PORT}
#tika
tks.host={TKS_HOST}
tks.port={TKS_PORT}

#Normalize log configuration path.
log4rs.config={SERVICE_LOG4RS}
"#;

pub(crate) const FILE_SERVER_APP_PROPERTIES_TEMPLATE: &str = r#"
#Rocket server port
server.port={SERVICE_PORT}
app.ek={SERVICE_CEK}

#database source
db.hostname={DB_HOST}
db.port={DB_PORT}
db.name=ad_{DOKA_INSTANCE}
db.user={DB_USER}
db.password={DB_PASSWORD}
db.pool_size=10

#Key Manager service
km.host={KM_HOST}
km.port={KM_PORT}
#Session Manager service
sm.host={SM_HOST}
sm.port={SM_PORT}
#Document Server
ds.host={DS_HOST}
ds.port={DS_PORT}
#tika
tks.host={TKS_HOST}
tks.port={TKS_PORT}

#Normalize log configuration path.
log4rs.config={SERVICE_LOG4RS}
"#;

pub (crate) const LOG4RS_TEMPLATE : &str = r#"
refresh_rate: 10 seconds

appenders:
  console:
    kind: console
    encoder:
      pattern: "{d(%+)(local)} [{t}] {h({l})} [{M}] {m} [EOL] {n}"
#     https://docs.rs/log4rs/0.11.0/log4rs/encode/pattern/index.html
    filters:
      - kind: threshold
        level: info
  file:
    kind: file
    path: {LOG_FOLDER}
    encoder:
      pattern: "{d(%+)(local)} [{t}] {h({l})} [{M}] {m} [EOL] {n}"

root:
  level: debug
  appenders:
    - console
    - file

loggers:
  test::a:
    level: debug
    appenders:
      - file
    additive: true
"#;