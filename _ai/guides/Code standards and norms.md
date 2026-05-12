### Code standards and norms 

| `DOKA_ONE`

---

* **[SES_CHECK] session verification**

The first thing to do in the a delegate is to check is the session token is valid 

```rust
// Check if the token is valid
let entry_session = try_or_return!(
    valid_sid_get_session(&self.session_token, &mut self.follower).await,
    Self::web_type_error()
);      
```

Then you can grab the session id (sid) from id 

```rust
let customer_code = entry_session.customer_code.as_str();
```

If we simply have a security token , we simply check it the same way 

```rust
// Check if the token is valid
if !self.security_token.is_valid() {
    log_error!("💣 Invalid security token, follower=[{}]", &self.follower);
    return WebType::from_errorset(&INVALID_TOKEN);
}

self.follower.token_type = TokenType::Token(self.security_token.0.clone());
```

---

* **[ERR_REPLY] predefined error codes**

For each reply type (ex: GetFileInfoShortReply), we can use  WebType<GetFileInfoShortReply> . 

```rust
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct GetFileInfoShortReply {
    pub file_ref : String,
    pub original_file_size: u64,
    pub block_count : u32,
    pub encrypted_count : i64,
    pub fulltext_indexed_count : i64,
    pub preview_generated_count : i64,   
}

pub async fn file_stats(
    session_token: SessionToken,
    Path(file_ref): Path<String>,
) -> WebType<GetFileInfoShortReply>
```

It will allow us to use standard implemented reply errors like 

```rust
WebType::from_errorset(&INVALID_TOKEN);
```

---

* **[ERR_MGT] all error management**

1. In the delegate of the entry point the error must be totally caught, any routine that return a Result<> must be followed by a map_err() and a err_fwd!() with a clear error message.

err_fwd!() is a macro that shows the line number and the source file name in the message.

```rust
let Ok(user_id) = sql_insert.insert(&mut trans)
					.map_err(err_fwd!("Insertion of a new admin user failed, follower=[{}]", &self.follower)) 
else {
    let _ = warning_cs_schema(&customer_code);
    let _ = warning_fs_schema(&customer_code);
    return Json(CreateCustomerReply::internal_database_error_reply());
};
```

2. In a sub routine (meaning that are not the main delegate routine of the entry point), you can either do the same (map_err + err_fwd! )

```rust
let clear_customer_key = DkEncrypt::decrypt_str(&customer_key, &cek)
                                    .map_err(err_fwd!("Cannot decrypt the customer key, follower=[{}]", &self.follower))?;
```

or you can simply show the line number by using tr_fwd!

```rust
let km_host = get_prop_value(KEY_MANAGER_HOSTNAME_PROPERTY).map_err(tr_fwd!())?;
```

In the case of Option<>, you can use `ok_or()` that returns `Result<>` if the option is `None`

```rust
let key_not_found = anyhow::anyhow!("Cannot find the customer key");
let customer_key = response.keys.get(customer_code).ok_or(key_not_found)?.ciphered_key.as_str();
```

---

* **[START_END] logs (log_info!( "start...) and "end..." messages, etc)**

```rust
log_info!("🚀 Start {}", PROGRAM_NAME);
```

```
log_info!("🏁 End {}", PROGRAM_NAME);
```

---

* **[SQ_BRA] Squared brackets for log values**

```rust
log_info!("The CEK was correctly read : [{}]", format!("{}...", &new_prop[0..5]));
```

---

* **[REL_STEPS] Main and delegate api routine logs with icons**

The main delegate of the end point must trace every relevant step of the processing with clear log_xxx  + icons

```rust
log_info!("😎 Security token is valid, follower=[{}]", &self.follower);
```

```rust
log_error!("💣 Create customer failed, follower=[{}]", &self.follower);
```

In case of key processing, we can use the "saint" icon, which will be ended by the sunglasses icon :  

```rust
log_info!("😇 Normalize lexeme : {}", &TokenSlice(&tokens));
```

```rust
log_info!("😎 Final normalisation : {}", &TokenSlice(&tokens));
```

---

* **[FOLLOWER] follower, x_request_id**

A Follower has 2 attributes : 

-- XRequestID which is a constant number all along the service execution and across the other extra service calls.

-- A TokenType which is either a Token, a Sid or None, and contains the encrypted string used for the service security.

Considering the delegate type has a Follower, we can make sure that the included x_request_id has a correct value, meaning, if not null, we must keep it otherwise, we must generate it. We can do that in the Delegate's new() , like below:

```rust
x_request_id : x_request_id.new_if_null(),
```

---

* **[SEP_DEL] All end points in the main file, delegates in other files** 

The main.rs file will define the entry points for web service but the actual implementation in the delegates will take place in extra files like customer.rs

---

* **[DEL_T]  Delegate type** 

The end point must create a delegate type and call the associate routine

```rust
#[post("/customer", format = "application/json", data = "<customer_request>")]
pub async fn create_customer(customer_request: Json<CreateCustomerRequest>, security_token: SecurityToken, x_request_id: XRequestID) -> Json<CreateCustomerReply> {
    let mut delegate = CustomerDelegate::new(security_token, x_request_id);
    delegate.create_customer(customer_request).await
}
```

---

* **[WS_CAT] Web service category icon**

A web service that needs a token will start its doc with:  🔑
A web service that needs a session id will start its doc with:  🌟 (or older ✨)
A web service that doesn't need any token nor session id its doc with:  0️

---

