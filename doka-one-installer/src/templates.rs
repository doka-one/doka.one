
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

pub(crate) const KM_APP_PROPERTIES_TEMPLATE: &str = r#"
#Rocket server port
server.port={KM_PORT}
app.ek={KM_CEK}

#database source
db.hostname={DB_HOST}
db.port={DB_PORT}
db.name=ad_{DOKA_INSTANCE}
db.user={DB_USER}
db.password={DB_PASSWORD}
db.pool_size=10

#Normalize log configuration path.
log4rs.config={KM_LOG4RS}
"#;