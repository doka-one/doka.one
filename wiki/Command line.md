## Doka Command Line 

This document is eventually meant to be a web page published on the doka web site.

You can use the command line in direct mode or interactive mode.

---

**Generate a temporary token**

```bash
dk token generate
        -c  --cek  c:\install\doka.one\config\cek.key
```

Security token : {"expiry_date": "2022-12-05T12:00:00Z"}  
49 minutes left....  
j6nk2GaKdfLl3nTPb... 

> This token contains a timestamp and the validation makes sure the key is not outdated 

---

**Create a domain [token]**

```bash
dk domain create    
		-d  --domain_name dev_1
```

Domain created: dev_1

> This command will use the temporary generated key

---

**Create a customer [token]**

```bash
dk customer create    
		-n  --customer_name denis@isd.lu    
		-e  --email denis@isd.lu  
		-ap --admin_password <user_passord>   
```

Customer created, code : fd2da7e1
User created:  denis@isd.lu

> This command will use the temporary generated key

---

**Disable a customer [token]**

```bash
dk customer disable 
    -c --customer_code  fd2da7e1
```

Customer disabled

---

**Login into the system**

```bash
dk session login 
    -u --username denis@isd.lu  		
    -p --password '<user_passord>' 
```

Session opened with token : lkasjdf4sdfa6[...]

> Ensure the user and password is valid and generate a local session id usable by the session commands

---

**Upload a file [session]**

> Upload a binary content to doka.

```bash
dk file upload 
    -pt --path "c:\data\upload\planet.pdf"
    -ii --item-info "planet_1234"
```

Loaded file :  planet.pdf, reference: AFGF456

<table>
    <tr>
        <td><b>Feature</b></td><td><b>Command Line</b></td><td><b>Unit Test</b></td>
    </tr>
    <tr>
        <td>Upload the file blocks</td><td>👍</td><td>👍</td>
    </tr>
    <tr>
        <td>Read and store the metadata</td><td>👍</td><td>❌</td>
    </tr>
    <tr>
        <td>Parse and store the text content</td><td>👍</td><td>👍</td>
    </tr>
</table>

---

**Uploads statistics [session]**

```bash
dk file loading     
```
> Show all the data about the ongoing uploads for the user.

Completion (%) | Parts | Meta |  Encryption | Text parsing | 
45%             5/9     Done    Pending      Pending

<table>
    <tr>
        <td><b>Feature</b></td><td><b>Client</b></td><td><b>Command Line</b></td><td><b>Unit Test</b></td>
    </tr>
    <tr>
        <td>Show all the data</td><td>👍</td><td>👍</td><td>❌</td>
    </tr>
</table>


---

**Info about a specific file during the loading process [session]**

```bash
dk file stats
    -fr --file_reference AFGF456       
```

> Display the information about a specific file being loaded

unknown

<table>
    <tr>
        <td><b>Feature</b></td><td><b>Client</b></td><td><b>Command Line</b></td><td><b>Unit Test</b></td>
    </tr>
    <tr>
        <td>Read the information about the file parts</td><td>👍</td><td>❌</td><td>👍</td>
    </tr>
</table>

---

**Info about a file [session]**

```bash
dk file info
    -fr --file_reference AFGF456       
```
> Display the information about the loaded file.

<table>
    <tr>
        <td><b>Feature</b></td><td><b>Client</b></td><td><b>Command Line</b></td><td><b>Unit Test</b></td>
    </tr>
    <tr>
        <td>Read the information about the file parts</td><td>👍</td><td>👍</td><td>👍</td>
    </tr>
    <tr>
        <td>Read the metadata information</td><td>❌</td><td>❌</td><td>❌</td>
    </tr>
</table>

----

**List of files [session]**

```bash
dk file list
    -m --match *e35*
```

> List the existing files that match the expression.

<table>
    <tr>
        <td><b>Feature</b></td><td><b>Client</b></td><td><b>Command Line</b></td><td><b>Unit Test</b></td>
    </tr>
    <tr>
        <td>List the existing file</td><td>👍</td><td>👍</td><td>❌</td>
    </tr>
    <tr>
        <td>Filter on the provided expression</td><td>👍</td><td>👍</td><td>❌</td>
    </tr>
</table>



---

**Download a file [session]**

```bash
dk file download
    -fr --file_reference AFGF456 
    -pt --path 'c:\data\upload\planet.pdf'  
```

Downloaded file :  planet.pdf, reference: AFGF456

> Download the binary content of a file and save it in the file path.  

---
**Create items [session]**

> Create an item in the system with properties and the document attached
> The command can take optional file_reference or a path to a file.
> When no file or path is provided, the item will not be linked to any existing file

```bash
dk item create
    -n  --name  "image402.jpg"
    [-p  --property "(category:astro)(email_number:789845)"]    
    [-fr --file_reference "AFGF456"]            The reference of the file already in the system
    [-pt --path "c:\data\upload\planet.pdf"]    The file to be uploaded
```

Created item with id : 459

---
**Add tags on item [session]**

> Add a list of tags to the item.
> The command take the identifier of the item and a list of tags.

```bash
dk item tag
    -id  459
    -u  --update "(category:astro)(email_number:789845)"    
```

Tags added to item with id : 459

* text, bool, int, decimal, date, datetime, link are the possible type to use for a tag.
*  "(category:bureau)(origin:2024-10-01:date)"  the type can be placed in the third section of the tag definition, "text" is implicit.
* The "link" type is used to establish a connection to another item :  (original_document:4:link)

----
**Delete tags on item [session]**

> Delete a list of tags from the item.
> The command take the identifier of the item and a list of tags.

```bash
dk item tag
    -id  459
    -d  --delete "category,email_number"    
```

Tags deleted from item with id : 459

----
**Search items [session]**

```bash
dk item search 
    -f --filter "file_name=*planet*"
```

id:459   planet.pdf       category:astro       email_number:789845

> List all the item matching the properties (**todo**)

---

**Search items [session]**

```bash
dk item get 
    -id --identifier 26
```

id:459   planet.pdf       category:astro       email_number:789845

> Show the item and its properties





