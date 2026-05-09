doka one

This project is a reboot of doka live.
It aims at simplifying the features and the development process.

Features:

* A user, being part of a company, can import any kind of files in the system
* Those files will be parsed and stored, and a list of tags and properties will be proposed
* After assigning the tags to a file, it can be marked as "imported" and become part of the system
* A file's tag can be changed at anytime
* We can search for a file from its content and its tags
* We can create a permanent "drawer" from a previous search. A drawer can have group levels.
* There is no admin of the system, a user is its own admin.
* A user can enter its IMAP email account parameters in order to feed its system


* From the login page, a person can request an access to doka one by filling a small form and checking a captcha.
* A security code will be sent by email
* The person enters the security code on a confirmation webpage.
* A unique user code is assigned to the person. 
* A free user is limited in storage capacity. We can see it on the website's account page.

### Usuage service ports

````
Ports : 
key-manager : 30040 
session-manager: 30050  
admin-server : 30060
document-server : 30070 
file-server: 30080
tika-server : 40010
````

Modules :

* key-manager
* session-manager
* admin-server
* document-server
* file-server
  * Upload
  * Download
* parser-server (tika)
* email-server
* unit-tests
* doka-cli
* doka-front

Parsing an incoming email 

--- Phase 1 : Pre-processing ---
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
  

--- PHASE 2 ----
  Full text processing for each new document (emails, attachments)

--- PHASE 3 ----
  Content matching and tag association

---
Database tools
https://app.sqldbm.com/SQLServer/DatabaseExplorer/p202076#

---
