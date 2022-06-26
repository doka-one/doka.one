use std::collections::HashMap;
use std::io::Read;
use std::{io, thread};
use std::fs::File;
use std::thread::JoinHandle;
use anyhow::anyhow;
use rocket::Data;
use rocket::http::{ContentType, RawStr};
use rocket::response::Content;
use rocket_contrib::json::Json;
use rs_uuid::iso::uuid_v4;
use rustc_serialize::base64::{ToBase64, URL_SAFE};
use log::{info, debug, warn, error};
use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock};
use commons_services::database_lib::open_transaction;
use commons_services::key_lib::fetch_customer_key;
use commons_services::property_name::{DOCUMENT_SERVER_HOSTNAME_PROPERTY, DOCUMENT_SERVER_PORT_PROPERTY, TIKA_SERVER_HOSTNAME_PROPERTY, TIKA_SERVER_PORT_PROPERTY};
use commons_services::session_lib::fetch_entry_session;
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::error_replies::ErrorReply;
use dkdto::{BlockStatus, GetFileInfoReply, GetFileInfoShortReply, JsonErrorSet, UploadReply};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, SUCCESS};
use doka_cli::request_client::{DocumentServerClient, TikaServerClient, TokenType};

#[derive(Debug,Clone)]
pub(crate) struct FileDelegate {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl FileDelegate {

    const BLOCK_SIZE : usize = 1_048_576;
    // const BLOCK_SIZE : usize = 2_000;

    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            }
        }
    }


    ///
    /// ✨ Upload the binary content of a file
    ///
    /// Split into parts and store them (parallel)
    /// Call tika
    /// Call document_server.ft_indexing() (parallel process)
    ///
    /// TODO keeping the entire binary content in memory is not a neat idea
    ///     please, store the parts in the db and pass the file handle around.
    pub fn upload(&mut self, file_data: Data) -> Json<UploadReply> {

        log_info!("🚀 Start upload api, follower=[{}]", &self.follower);

        // Check if the token is valid
        if !self.session_token.is_valid() {
            if cfg!(windows) {
                self.empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            log_error!("💣 Invalid session token, token=[{:?}], follower=[{}]", &self.session_token, &self.follower);
            return Json(UploadReply::invalid_token_error_reply());
        }

        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        let internal_database_error_reply = Json(UploadReply::internal_database_error_reply());
        let internal_technical_error = Json(UploadReply::internal_technical_error_reply());

        // Read the session information
        let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value()).map_err(err_fwd!("💣 Session Manager failed, follower=[{}]", &self.follower)) else {
            if cfg!(windows) {
                self.empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            return internal_technical_error;
        };

        let customer_code = entry_session.customer_code.as_str();

        // Get the crypto key

        let Ok(customer_key) = fetch_customer_key(customer_code, &self.follower.token_type.value())
                                    .map_err(err_fwd!("💣 Cannot get the customer key, follower=[{}]", &self.follower)) else {
            if cfg!(windows) {
                self.empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            return internal_technical_error;
        };

        // Create an entry in file_reference
        let mut r_cnx = SQLConnection::new();

        let Ok(( file_id, file_ref )) = self.create_file_reference(&mut r_cnx, customer_code)
                                .map_err(err_fwd!("💣 Cannot create an entry in the file reference table")) else {
            if cfg!(windows) {
                self.empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            return internal_database_error_reply;

        };

        log_info!("😎 Created entry in file reference, file_id=[{}], file_ref=[{}], follower=[{}]", file_id, &file_ref, &self.follower);

        // Create parts

        log_info!("Start creating parts in parallel, follower=[{}]", &self.follower);

        let mut mem_file : Vec<u8> = vec![];
        let mut thread_pool = vec![];

        const MAX_BUF : usize = 1_024;
        // const MAX_BUF : usize = 600;
        let mut block : [u8; Self::BLOCK_SIZE] = [0; Self::BLOCK_SIZE];
        let mut block_index : usize = 0;
        let mut block_num : u32 = 0;
        let mut total_size : usize = 0;
        let mut datastream = file_data.open();
        let mut block_set : HashMap<u32, Vec<u8>> = HashMap::new();

        loop {
            let mut buf : [u8; MAX_BUF] = [0; MAX_BUF];
            let r_bytes = datastream.read(&mut buf);

            match r_bytes {
                Ok(nb_bytes) => {
                    // log_info!("Ok : [{:?}]", &buf);
                    if nb_bytes == 0 {
                        break;
                    } else {

                        for b_index in 0..nb_bytes {
                            if block_index >= Self::BLOCK_SIZE {
                                let slice = &block[0..block_index];
                                block_set.insert(block_num, slice.to_vec());
                                if block_set.len() >= 10 {
                                    thread_pool.push(
                                        self.parallel_crypto_and_store_block(file_id, &block_set,  customer_code, &customer_key)
                                    );
                                    block_set.clear();
                                }

                                total_size += block_index;
                                block_num += 1;
                                block = [0; Self::BLOCK_SIZE];
                                block_index = 0;
                            }

                            let x  = buf[b_index];
                            block[block_index] = x;
                            block_index += 1;

                            // Store the byte in a memory file
                            mem_file.push(x);
                        }

                    }
                }
                Err(_) => {
                    break;
                }
            }
        }

        log_info!("Start encrypting the block in parallel, follower=[{}]", &self.follower);

        if block_index > 0 {
            let slice = &block[0..block_index];
            log_info!("End slice : {} {}", block_index, slice.len());
            block_set.insert(block_num, slice.to_vec());


            thread_pool.push(self.parallel_crypto_and_store_block(file_id, &block_set,/*block_num, slice,*/
                                                             customer_code,
                                                             &customer_key));
            block_set.clear();

            total_size += block_index;
            block_num += 1;
        }

        let original_file_size = mem_file.len();

        // Create the content parsing process
        // | We know that all the blocks have been read and the mem_file contains all the data.
        // | We don't know the state of each parts
        // | But still we know the original_file_size (mem_file.len()) and the total_part (block_num)
        thread_pool.push(self.parallel_parse_content(&file_ref, mem_file, customer_code));

        let local_follower = self.follower.clone();
        thread::spawn( move || {
            for th in thread_pool {
                if let Err(e) = th.join() {
                    log_error!("Thread join error [{:?}], follower=[{}]", e, &local_follower);
                }
            }
        });

        log_info!("Start updating the file reference, follower=[{}]", &self.follower);

        // Update the file_reference table : checksum, original_file_size, total_part
        if self.update_file_reference(&mut r_cnx, file_id, original_file_size, block_num, customer_code)
            .map_err(err_fwd!("Cannot create an entry in the file reference table, follower=[{}]", &self.follower)).is_err() {
            return internal_database_error_reply;
        }

        log_info!("🏁 End upload api, follower=[{}]", &self.follower);

        Json(UploadReply {
            file_ref,
            size : total_size,
            block_count: block_num,
            status: JsonErrorSet::from(SUCCESS),
        })
    }


    // Windows only
    fn empty_datastream(&self, reader : &mut dyn Read) {
        // TODO test it on linux!
        // BUG https://github.com/SergioBenitez/Rocket/issues/892
        log_warn!("⛔ Running on Windows, need to read the datastream");
        let _r = io::copy( reader, &mut io::sink());
    }


    ///
    /// Run a thread to process the block and store it in the DB
    ///
    fn parallel_crypto_and_store_block(&self, file_id : i64, block_set : &HashMap<u32, Vec<u8>>,
                                       customer_code : &str,
                                       customer_key : &str) -> JoinHandle<anyhow::Result<()>> {

        let s_customer_code = customer_code.to_owned();
        let s_customer_key = customer_key.to_owned();
        let local_block_set = block_set.into_iter()
            .map(|(key, value)| { (*key, (*value).to_owned())  })
            .collect();


        let local_self = self.clone();

        let th = thread::spawn( move || {
            local_self.crypto_and_store_block( file_id, local_block_set,
                                    s_customer_code, s_customer_key)
        });

        th
    }

    fn parallel_parse_content(&self, file_ref: &str, mem_file : Vec<u8>, customer_code: &str) -> JoinHandle<anyhow::Result<()>> {

        let my_file_ref = file_ref.to_owned();
        // let my_sid = self.follower.token_type.value().to_owned();
        let my_customer_code = customer_code.to_owned();

        let local_self = self.clone();

        let th = thread::spawn( move || {
            local_self.parse_content(&my_file_ref, mem_file,  &my_customer_code)
        });

        th
    }

    fn min_max<T>(map : &HashMap<u32, T> ) ->  (u32,u32) {
        let mut min : u32 = u32::MAX;
        let mut max : u32 = u32::MIN;

        for (index, _) in map {
            if *index >= max {
                max = *index;
            }
            if *index <= min {
                min = *index;
            }
        }

        (min, max)
    }


    //
    fn crypto_and_store_block(&self, file_id : i64, block_set : HashMap<u32, Vec<u8>>,
                              customer_code: String, customer_key: String) -> anyhow::Result<()> {

        // Open the transaction
        let block_range = Self::min_max(&block_set);

        log_info!("Block range processing, block range=[{:?}], follower=[{}]", &block_range, &self.follower);

        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, block_range=[{:?}], follower=[{}]", &block_range, &self.follower))?;

        for (block_num, block) in block_set {

            log_debug!("Block processing... : block_num=[{}], follower=[{}]", block_num, &self.follower);

            // Encrypt the block
            let encrypted_block = DkEncrypt::encrypt_vec(&block, &customer_key)
                .map_err(err_fwd!("Cannot encrypt the data block, follower=[{}]", &self.follower))?;

            // and store in the DB

            let data = encrypted_block.to_base64(URL_SAFE);

            let sql_query = format!(r"
                    INSERT INTO fs_{}.file_parts (file_reference_id, part_number, is_encrypted, part_data)
                    VALUES (:p_file_reference_id, :p_part_number, :p_is_encrypted, :p_part_data)", customer_code);

            let sequence_name = format!("fs_{}.file_parts_id_seq", customer_code);

            let mut params = HashMap::new();
            params.insert("p_file_reference_id".to_string(), CellValue::from_raw_int(file_id));
            params.insert("p_part_number".to_string(), CellValue::from_raw_int_32(block_num as i32));
            params.insert("p_is_encrypted".to_string(), CellValue::from_raw_bool(true));
            params.insert("p_part_data".to_string(), CellValue::from_raw_string(data));

            let sql_insert = SQLChange {
                sql_query,
                params,
                sequence_name,
            };

            let _file_part_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

            log_debug!("...Block inserted, block_num=[{}], follower=[{}]", block_num, &self.follower);

        }

        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!("😎 Committed. Block inserted, block_range=[{:?}], follower=[{}]", &block_range, &self.follower);

        Ok(())
    }

    ///
    /// Call the tika server to parse the file and get the text data
    /// Call the document server to fulltext parse the text data
    ///
    fn parse_content(&self, file_ref: &str, mem_file : Vec<u8>, customer_code: &str) -> anyhow::Result<()> {

        log_info!("Parsing file content ... ,file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        let tika_server_host = get_prop_value(TIKA_SERVER_HOSTNAME_PROPERTY)?;
        let tika_server_port = get_prop_value(TIKA_SERVER_PORT_PROPERTY)?.parse::<u16>()?;
        let document_server_host = get_prop_value(DOCUMENT_SERVER_HOSTNAME_PROPERTY)?;
        let document_server_port = get_prop_value(DOCUMENT_SERVER_PORT_PROPERTY)?.parse::<u16>()?;

        // Get the raw text from the original file
        let tsc = TikaServerClient::new(&tika_server_host, tika_server_port);
        let raw_text = tsc.parse_data(&mem_file).map_err(err_fwd!("Cannot parse the original file"))?;

        log_debug!("Parsing done for file_ref=[{}], content size=[{}], follower=[{}]", file_ref, raw_text.x_tika_content.len(), &self.follower);

        let document_server = DocumentServerClient::new(&document_server_host, document_server_port);
        // TODO we must also pass the  self.follower.x_request_id
        let reply = document_server.fulltext_indexing(&raw_text.x_tika_content,
                                                      "no_filename_for_now",
                                                      file_ref,
                                                      &self.follower.token_type.value());

        if reply.status.error_code == 0 {
            log_info!("Fulltext indexing done, number of text parts=[{}], follower=[{}]", reply.part_count, &self.follower);
            self.set_file_reference_fulltext_indicator(file_ref, customer_code)
                            .map_err(err_fwd!("Cannot set the file reference to fulltext parsed indicator, follower=[{}]", &self.follower))?;
        } else {
            log_error!("Error while sending the raw text to the fulltext indexing, file_ref=[{}], reply=[{:?}], follower=[{}], ",
                                                                                                                        file_ref, reply.status, &self.follower);
            return Err(anyhow::anyhow!(reply.status.err_message));
        }

        log_info!("... End of parse file content processing, file_ref=[{}], follower=[{}]", file_ref, &self.follower);
        Ok(())
    }


    //
    fn set_file_reference_fulltext_indicator(&self, file_ref: &str, customer_code: &str) -> anyhow::Result<()> {

        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, follower=[{}]", &self.follower))?;

        let sql_query = format!(r"UPDATE fs_{}.file_reference
                SET is_fulltext_parsed = true
                WHERE file_ref = :p_file_ref ", customer_code);

        let sequence_name = format!( "fs_{}.file_reference_id_seq", customer_code );

        let mut params = HashMap::new();
        params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));

        let sql_update = SQLChange {
            sql_query,
            params,
            sequence_name,
        };

        let _ = sql_update.update(&mut trans).map_err(err_fwd!("Update failed, follower=[{}]", &self.follower))?;

        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!("😎 Committed. Successfully set the full text indicator, file_ref=[{:?}], follower=[{}]", file_ref, &self.follower);

        Ok(())
    }


    ///
    /// ✨ Get the information about the composition of a file [file_ref]
    ///
    pub fn file_info(&mut self, file_ref: &RawStr) -> Json<GetFileInfoReply> {

        log_info!("🚀 Start upload api, follower=[{}]", &self.follower);

        // Check if the token is valid
        if !self.session_token.is_valid() {
            log_error!("💣 Invalid session token, token=[{:?}], follower=[{}]", &self.session_token, &self.follower);
            return Json(GetFileInfoReply::invalid_token_error_reply());
        }
        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        // Read the session information


        let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value())
                                    .map_err(err_fwd!("💣 Session Manager failed, follower=[{}]", &self.follower)) else {
            return Json(GetFileInfoReply::internal_technical_error_reply());
        };

        let customer_code = entry_session.customer_code.as_str();

        let mut block_status = vec![];


        let sql_query = format!(r"SELECT fp.id, fr.file_ref, fp.part_number,
            fp.is_encrypted,
            fr.is_fulltext_parsed,
            fr.is_preview_generated
        FROM  fs_{}.file_reference fr, fs_{}.file_parts fp
        WHERE
            fp.file_reference_id = fr.id AND
            fr.file_ref = :p_file_reference
        ORDER BY fr.file_ref, fp.part_number", customer_code, customer_code);

        let r_data_set : anyhow::Result<SQLDataSet> = (|| {
            let mut r_cnx = SQLConnection::new();
            let mut trans = open_transaction(&mut r_cnx)?;

            let mut params = HashMap::new();
            params.insert("p_file_reference".to_string(), CellValue::from_raw_string(file_ref.to_string()));

            let query = SQLQueryBlock {
                sql_query,
                start: 0,
                length: None,
                params,
            };

            let dataset = query.execute(&mut trans)?;

            trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

            Ok(dataset)

        })();

        let Ok(mut data_set) = r_data_set else {
            return Json(GetFileInfoReply::internal_database_error_reply());
        };

        // inner function
        fn build_block_info(data_set: &mut SQLDataSet) -> anyhow::Result<BlockStatus> {
            let block_number = data_set.get_int_32("part_number").ok_or(anyhow!("Wrong part_number col"))? as u32;
            let is_encrypted = data_set.get_bool("is_encrypted").ok_or(anyhow!("Wrong is_encrypted col"))?;
            let is_fulltext_indexed = data_set.get_bool("is_fulltext_indexed").ok_or(anyhow!("Wrong is_fulltext_indexed col"))?;
            let is_preview_generated = data_set.get_bool("is_preview_generated").ok_or(anyhow!("Wrong is_preview_generated col"))?;
            //let part_length = data_set.get_int_32("part_length").unwrap_or(-1) as u32;

            Ok(BlockStatus {
                original_size: 0,
                block_number,
                is_encrypted,
                is_fulltext_indexed,
                is_preview_generated
            })
        }

        while data_set.next() {
            match build_block_info(&mut data_set) {
                Ok(block_info) => {
                    block_status.push(Some(block_info));
                }
                Err(e) => {
                    log_warn!("⛔ Warning while building block info, e=[{}], follower=[{}]", e, &self.follower)
                }
            }

        }

        log_info!("🏁 End file_info api, follower=[{}]", &self.follower);

        Json(GetFileInfoReply {
            file_ref : file_ref.to_string(),
            block_count: block_status.len() as u32,
            block_status,
            status: JsonErrorSet::from(SUCCESS),
        })
    }


    ///
    /// ✨ Get the information about the loading status of the [file_ref]
    ///
    pub fn file_stats(&mut self, file_ref: &RawStr) -> Json<GetFileInfoShortReply> {

        log_info!("🚀 Start upload api, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        // Check if the token is valid
        if !self.session_token.is_valid() {
            log_error!("💣 Invalid session token, token=[{:?}], follower=[{}]", &self.session_token, &self.follower);
            return Json(GetFileInfoShortReply::invalid_token_error_reply());
        }
        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        // Read the session information
        let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value()).map_err(err_fwd!("💣 Session Manager failed, follower=[{}]", &self.follower)) else {
             return Json(GetFileInfoShortReply::internal_technical_error_reply());
        };

        let customer_code = entry_session.customer_code.as_str();

        // TODO instead of constant 1, check if the document is fulltext parsed and previewed
        let sql_query = format!(r"
                SELECT fr.mime_type, fr.checksum, fr.original_file_size, fr.total_part, enc.encrypted, fulltext.fulltext, preview.preview
                    FROM fs_{}.file_reference fr,
                    (SELECT count(*) as encrypted
                        FROM  fs_{}.file_parts
                        WHERE file_reference_id = (SELECT id FROM fs_{}.file_reference WHERE file_ref = :p_file_ref)
                        AND is_encrypted = true) enc,
                    1 fulltext,
                    1 preview
                    WHERE file_ref = :p_file_ref ", customer_code, customer_code, customer_code );

        let r_data_set : anyhow::Result<SQLDataSet> = (|| {
            let mut r_cnx = SQLConnection::new();
            let mut trans = open_transaction(&mut r_cnx)?;

            let mut params = HashMap::new();
            params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));

            let query = SQLQueryBlock {
                sql_query,
                start: 0,
                length: None,
                params,
            };

            let dataset = query.execute(&mut trans)?;
            trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

            Ok(dataset)
        })();

        let Ok(mut data_set) = r_data_set else {
            return Json(GetFileInfoShortReply{
                file_ref: "".to_string(),
                original_file_size : 0,
                block_count: 0,
                encrypted_count: 0,
                fulltext_indexed_count: 0,
                preview_generated_count: 0,
                status: JsonErrorSet::from(INTERNAL_DATABASE_ERROR),
            })
        };


        // inner function
        fn build_file_info(data_set: &mut SQLDataSet, file_ref: &str) -> anyhow::Result<GetFileInfoShortReply> {
            let _mime_type = data_set.get_string("mime_type").ok_or(anyhow!("Wrong mime_type col"))?;
            let _checksum = data_set.get_string("checksum").ok_or(anyhow!("Wrong checksum col"))?;
            let original_file_size = data_set.get_int("original_file_size").ok_or(anyhow!("Wrong original_file_size col"))?;
            let total_part = data_set.get_int_32("total_part").ok_or(anyhow!("Wrong total_part col"))?;
            let encrypted_count = data_set.get_int("encrypted").ok_or(anyhow!("Wrong encrypted col"))?;
            let fulltext_indexed_count = data_set.get_int("fulltext").ok_or(anyhow!("Wrong fulltext col"))?;
            let preview_generated_count = data_set.get_int("preview").ok_or(anyhow!("Wrong preview col"))?;

            Ok(GetFileInfoShortReply{
                file_ref: file_ref.to_string(),
                block_count: total_part as u32,
                original_file_size : original_file_size as u64,
                encrypted_count,
                fulltext_indexed_count,
                preview_generated_count,
                status: JsonErrorSet::from(SUCCESS),
            })
        }


        let mut stats = GetFileInfoShortReply{
            file_ref: "".to_string(),
            original_file_size : 0,
            block_count: 0,
            encrypted_count: 0,
            fulltext_indexed_count: 0,
            preview_generated_count: 0,
            status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
        };

        if data_set.next() {

            let Ok(stats) = build_file_info(&mut data_set, file_ref) else {
              return Json(GetFileInfoShortReply::internal_database_error_reply());
            };

        }
        // else {
        //     stats = GetFileInfoShortReply{
        //         file_ref: "".to_string(),
        //         original_file_size : 0,
        //         block_count: 0,
        //         encrypted_count: 0,
        //         fulltext_indexed_count: 0,
        //         preview_generated_count: 0,
        //         status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
        //     };
        // }

        log_info!("😎 Successfully read the file stats, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        log_info!("🏁 End file_stats api, follower=[{}]", &self.follower);

        Json(stats)

    }


    ///
    /// ✨ Download the binary content of a file
    ///
    pub fn download(&mut self, file_ref: &RawStr) -> Content<Vec<u8>> {

        // Check if the token is valid
        log_info!("🚀 Start download api, file_ref = [{}], follower=[{}]", file_ref, &self.follower);
        //
        // // Create parts
        // log_info!("🏁 End upload api, sid={}", &sid);

        let mut file = File::open("c:/Users/denis/Dropbox/Upload/russian_planet.pdf").unwrap();

        let mut bytes = vec![];
        let _b = file.read_to_end(&mut bytes);

        log_info!("🏁 End download api, follower=[{}]", &self.follower);

        Content(ContentType::PDF, bytes)

    }


    ///
    ///
    ///
    fn create_file_reference(&self, r_cnx : &mut anyhow::Result<SQLConnection>, customer_code: &str) -> anyhow::Result<(i64, String)> {

        let mut trans = open_transaction(r_cnx).map_err(err_fwd!("Open transaction error, follower=[{}]", &self.follower))?;
        let file_ref = uuid_v4();

        let sql_query = format!(r"INSERT INTO fs_{}.file_reference
            ( file_ref, mime_type,  checksum, original_file_size,  encrypted_file_size,  total_part )
            VALUES ( :p_file_ref, :p_mime_type, :p_checksum, :p_original_file_size, :p_encrypted_file_size, :p_total_part)", customer_code);

        let sequence_name = format!( "fs_{}.file_reference_id_seq", customer_code );

        let mut params = HashMap::new();
        params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.clone()));
        // TODO get the actual mime type
        params.insert("p_mime_type".to_string(), CellValue::from_raw_string(String::from("text")));
        params.insert("p_checksum".to_string(), CellValue::String(None));
        params.insert("p_original_file_size".to_string(), CellValue::Int(None));
        params.insert("p_encrypted_file_size".to_string(), CellValue::Int(None));
        params.insert("p_total_part".to_string(), CellValue::Int32(None));

        let sql_insert = SQLChange {
            sql_query,
            params,
            sequence_name,
        };

        let file_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!("😎 Committed. Successfully created a file reference, file_ref=[{}], follower=[{}]", &file_ref, &self.follower);

        Ok((file_id, file_ref))
    }

    ///
    ///
    ///
    fn update_file_reference(&self, r_cnx : &mut anyhow::Result<SQLConnection>,
                             file_id : i64,
                             total_size: usize,
                             total_part: u32,
                             customer_code: &str) -> anyhow::Result<()> {

        // TODO check where the file_ref is actually created and stored ...
        let mut trans = open_transaction(r_cnx).map_err(err_fwd!("Open transaction error, follower=[{}]", &self.follower))?;

        let sql_query = format!(r"UPDATE fs_{}.file_reference
                                        SET original_file_size = :p_original_file_size, total_part = :p_total_part
                                        WHERE id = :p_file_id "
                                        , customer_code);

        let sequence_name = format!( "fs_{}.file_reference_id_seq", customer_code );

        let mut params = HashMap::new();
        params.insert("p_original_file_size".to_string(), CellValue::from_raw_int(total_size as i64));
        params.insert("p_total_part".to_string(), CellValue::from_raw_int_32(total_part as i32));
        params.insert("p_file_id".to_string(), CellValue::from_raw_int(file_id));

        let sql_update = SQLChange {
            sql_query,
            params,
            sequence_name,
        };

        let _ = sql_update.update(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        Ok(())
    }

}
