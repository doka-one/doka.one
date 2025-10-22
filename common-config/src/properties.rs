use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub static PROPS: Lazy<Arc<RwLock<HashMap<String, String>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

// "app.customerfile"
pub fn get_prop_value(prop_name: &str) -> Result<String> {
    let props = PROPS.read().unwrap();
    let v = props.get(prop_name).ok_or_else(|| anyhow!("Prop not found: [{}]", prop_name))?.trim().to_owned();
    Ok(v)
}

// Remplace complètement la map partagée
pub fn set_prop_values(new_props: HashMap<String, String>) {
    let mut props = PROPS.write().unwrap();
    *props = new_props;
}

// Définit ou met à jour une seule valeur
pub fn set_prop_value(prop_name: &str, value: &str) {
    if let Ok(mut props) = PROPS.write() {
        props.insert(prop_name.to_string(), value.to_string());
    } else {
        eprintln!("⚠️ Impossible d'acquérir le verrou d'écriture pour PROPS");
    }
}

///
/// Retourne la chaîne de connexion PostgreSQL et la taille du pool.
///
pub fn get_prop_pg_connect_string() -> Result<(String, u32)> {
    let db_hostname = get_prop_value("db.hostname").map_err(|_| anyhow!("Missing property: db.hostname"))?;
    let db_port = get_prop_value("db.port").map_err(|_| anyhow!("Missing property: db.port"))?;
    let db_name = get_prop_value("db.name").map_err(|_| anyhow!("Missing property: db.name"))?;
    let db_user = get_prop_value("db.user").map_err(|_| anyhow!("Missing property: db.user"))?;
    let db_password = get_prop_value("db.password").map_err(|_| anyhow!("Missing property: db.password"))?;

    let db_pool_size_str = get_prop_value("db.pool_size").map_err(|_| anyhow!("Missing property: db.pool_size"))?;

    let db_pool_size =
        db_pool_size_str.parse::<u32>().map_err(|_| anyhow!("Invalid pool size: {}", db_pool_size_str))?;

    let connect_string = format!("postgres://{}:{}@{}:{}/{}", db_user, db_password, db_hostname, db_port, db_name);

    Ok((connect_string, db_pool_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Petite aide pour partir d'une map vide avant chaque test
    fn reset_props() {
        let mut props = PROPS.write().unwrap();
        props.clear();
    }

    #[test]
    fn test_set_and_get_prop_value() {
        reset_props();

        set_prop_value("username", "alice");
        let val = get_prop_value("username").unwrap();

        assert_eq!(val, "alice");
    }

    #[test]
    fn test_get_prop_value_not_found() {
        reset_props();

        let result = get_prop_value("missing_key");

        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("Prop not found"));
    }

    #[test]
    fn test_set_prop_values_overwrites_existing() {
        reset_props();

        set_prop_value("username", "bob");
        assert_eq!(get_prop_value("username").unwrap(), "bob");

        let mut new_map = HashMap::new();
        new_map.insert("username".into(), "carol".into());
        new_map.insert("email".into(), "carol@example.com".into());

        set_prop_values(new_map);

        assert_eq!(get_prop_value("username").unwrap(), "carol");
        assert_eq!(get_prop_value("email").unwrap(), "carol@example.com");
    }

    #[test]
    fn test_multiple_set_prop_value() {
        reset_props();

        set_prop_value("key1", "val1");
        set_prop_value("key2", "val2");
        set_prop_value("key3", "val3");

        let props = PROPS.read().unwrap();
        assert_eq!(props.get("key1").unwrap(), "val1");
        assert_eq!(props.get("key2").unwrap(), "val2");
        assert_eq!(props.get("key3").unwrap(), "val3");
    }
}
