
#### Parsing an incoming email 

##### Phase 1 : Pre-processing ---
(
email-server/read - force the IMAP sync and parse the incoming messages, 
                      return a list of email numbers already parsed  
email-server/parse/<eml> - used when we want to import an email from the GUI  
)
* Get the email from the imap server
* Chunk the email into pieces => Main email (text + html) , Sub-emails (text + html), Attachments
* Check if the attachments are already in the system by checksum
  * If yes, keep the document reference for "link tags"
  * If not, create new documents in the system and keep the references
* Process sub-emails
  * Find the original email from the sender, receiver, datetime, subject and body 
  * If not found 
    * If Html, create a document from it (after quoted-printable decode if necessary)
    * It text, create a document from it
    * Tag the new document with sender, receiver, datetime
  * If found
    * Keep the email #
* Process the Main email
  * If Html, create a document from it (after quoted-printable decode if necessary)
  * It text, create a document from it
  * Tag the new document with sender, receiver, datetime, 
  * "tag-link" to attachments and to sub emails (to_email_648D)
* Tag the sub-emails with a reversed tag link (from_email_9845HK) 
  

##### PHASE 2 ----
  Full text processing for each new document (emails, attachments)

#####  PHASE 3 ----
  Content matching and tag association
