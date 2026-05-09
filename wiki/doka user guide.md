## ~~Doka User Guide~~

##### ~~Create a customer~~ 

~~Open a new windows command line (to enjoy the newly created DOKA_CLI_ENV), environment variable)~~

````bash
doka-cli token generate -c %DOKA_ENV%\document-server\keys\cek.key
````

---

~~**TODO** : the doka admin APIs must be accessible through an internal sysadmin user. The internal user has a password defined at the install procedure, and it is stored in a \keys\sysadmin.secret file  ( = encrypt("<file_content>", HASH(sys-password)))~~

~~==> uniformize the access to the sysadmin via the session-manager also, improve the locally generated token...~~

```bash
doka-cli session sysadmin -p "<sys-password>" --> call token generate API with the sys password
```

---

````bash
doka-cli customer create -n "Denis International Company" -e "denis.4@inc.com" -ap "Myadmin123;"
````

````bash
doka-cli session login -u denis.4@inc.com  -p "Myadmin123;"
````

### ~~Define and manipulate items~~

> ~~You can define an item with a couple of properties~~ 

````bash
doka-cli item create -n item3 -p "(my_email:t@t.com)(tag2:blabla)"
````

> ~~A special type of properties is a "link" which can have a name and a item id as target.~~
> ~~If you define such a link, the target item is not impacted, meaning it will not have a link property in return.~~

````bash
doka-cli item tag -id 2 -u "(to:1:link)"
````

### ~~Search items~~

```bash
doka-cli item search  
	-f "(lastname LIKE a%) OR (+postal_code = 34500 AND lastname LIKE %h%)" 
	-s "lastname desc"
```

> ~~Don't use simple or double quote with text operator.~~
> ~~Use a "+" sign in the front of very selective condition~~

### ~~Upload a file~~ 

> ~~To upload a file, simply use the file upload command with the path of your file and upload identifier.~~
> ~~This identifier is only here to understand what is being loaded especially if you have my file with the same name in your upload list.~~

````bash
doka-cli file upload -pt "C:\Users\denis\Dropbox\Upload\38M.m4v" -ii "item_name_sldjfhls"
````

~~After you have uploaded a file, an item is automatically associated to it, because a file cannot exist by itself without its metadata stored in an item (**todo**).~~



````bash
doka-cli item create -fr d2043bbb-f75e-45b8-7fcc-61c29649c74b -n rapport_activité -p "(private)(level:6)"
````

