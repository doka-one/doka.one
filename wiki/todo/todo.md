
### TODO


~~**TODO** : the doka admin APIs must be accessible through an internal sysadmin user. The internal user has a password defined at the install procedure, and it is stored in a \keys\sysadmin.secret file  ( = encrypt("<file_content>", HASH(sys-password)))~~

~~==> uniformize the access to the sysadmin via the session-manager also, improve the locally generated token...~~

```bash
doka-cli session sysadmin -p "<sys-password>" --> call token generate API with the sys password
```

---

````bash
doka-cli customer create -n "Denis International Company" -e "denis.4@inc.com" -ap "<sys-password>"
````

````bash
doka-cli session login -u denis.4@inc.com  -p "<sys-password>"
````


---------------
* Le mime type n'est pas récupérer à l'envoi du fichier
* La lecture des méta data sur les blocks ne sont là uniquement pour déterminer la langue du bloc

* tika/text is enough to get all the metadata along with the text content
  * curl http://localhost:40010/tika/text -H "Accept: application/json" -T ./petersens.jpg
       

* ~~Encrypt the tsvector and the text tokens~~ (Done)
* ~~Set the document status after the FT indexing~~
* Verify all the process is standardized 
  * 1A session verification
  * 2A predefined error codes
  * 3A all error management
  * 1B logs (info!( "start...) and "end..." messages, etc)
    * 2B [] for log values
    * 3B Main and delegate api routine logs with icons
    * 4B info logs after each relevant steps
    * 5B follower, x_request_id
  * 1C All end points in the main file, delegates in other files 
  * 1D Referenced in the api_status_reference.md
  
* Verify the entire schemas : FK, indexes, ...

* ~~Compilation warning~~
* ~~Implement the new customer creation with multiple DB schemas....~~
* Implement a customer unit tests (admin-server)
* Implement a full integration test for document upload

* Implement the file download service
* Implement the search document service with criteria
* Encrypt the app.properties's DB password with the CEK
* CEK Generator
* Implement the DELETE for key-manager/key,  
  with a is_removable column + Call it in the delete_customer API.
