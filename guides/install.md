## Doka Install procedure

### Windows

Install Postgresql 11 or +

* Get a PG user with admin privileges (Ex : user: john, password : doo)
* Download the unaccent_default.rules file : https://doka.one/artefacts/....
* Copy the  unaccent_default.rules  into the  <pg_install_folder>\share\tsearch_data

Download the doka_one_installer.exe (SHA-256 : ........... )

Run the doka-one-installer.exe  IN ADMINISTRATOR MODE

```bash
doka-one-installer.exe install --installation-path "D:/app/doka.one" --db-host "localhost" --db-port "5432" --db-user-name "john" --db-user-password "doo" --instance-name "test_2" --release-number "0.1.0"

doka-one-installer.exe install -i "D:/app/doka.one" -H "localhost" -P "5432" -u "john" -p "doo" -I "test_2" -r "0.1.0"
```

You can also omit the `--db-user-password`, the program will prompt you to enter it manually.

Phase 1: Enter the basic install information 

> Enter the installation path :  e:\doka.one <br>
> Enter Postgresql host name : localhost <br>
> Enter Postgresql user name : john<br>
> Enter Postgresql user password : doo<br>
> Enter Instance name (default: prod_1) :<br>

Phase 2: Verification 

> ....Installation path verified ✔<br>
> ....Postgresql connection verified ✔<br>

Phase 3: Download artefacts

> ....Download apache Tika server ... ✔ ... Found port 40'010 available<br>
> ....Download key-manager ... ✔ ... Found port 30'040 available<br>
> ....Download session-manager ... ✔ ... Found port 30'050 available<br>
> ....Download admin-server ... ✔ ... Found port 30'060 available<br>
> ....Download document-server ... ✔ ... Found port 30'070 available<br>
> ....Download file-server ... ✔ ... Found port 30'080 available<br>
> ....Download doka-cli ... ✔<br>

Phase 4: Initialization

> Initialize administration schema (ad_prod_1) ... ✔<br>
> 
> Generate master key ... ✔ <br>
> 
> Building configuration files for micro-services ... ✔<br>
>
> Building windows services ... ✔<br>

Phase 5: Start up services

* Apache Tika Server is running on port : 40'010 (see logs in e:\doka.one\logs\\tika.log) <br>
* key-manager is running on port 30'040 (see logs in e:\doka.one\logs\key-manager.log) <br>
* session-manager is running on port 30'050 (see logs in e:\doka.one\logs\admin-server.log) <br>
* admin-server is running on port 30'060  <br>
* document-server is running on port 30'070 <br>
* file-server is running on port 30'080  <br>

##### Start doka one

doka-cli doka start

##### Shutdown doka one

doka-cli doka shutdown

##### Create a customer

Open a new windows command line (to enjoy the newly created DOKA_CLI_ENV), environment variable)

````bash
doka-cli token generate -c %DOKA_ENV%\document-server\keys\cek.key
````

````bash
doka-cli customer create -n "Denis International Company" -e "denis.4@inc.com" -ap "Myadmin123;"
````

````bash
doka-cli item create -n item3 -p "(my_email:t@t.com)(tag2:blabla)"
````

// TODO  
````bash
doka-cli item create -n item4 -p "(to:x78978-546-6546:link)(birthdate:2019-10-01:date)"
````

````bash
doka-cli file upload -pt "C:\Users\denis\Dropbox\Upload\38M.m4v" -ii "item_name_sldjfhls"
````

````bash
doka-cli item create -fr d2043bbb-f75e-45b8-7fcc-61c29649c74b -n rapport_activité -p "(private)(level:6)"
````
##### Login

````bash
doka-cli session login -u denis.4@inc.com  -p "Myadmin123;"
````
##### Create an item

List all the items

##### Download an attached file

Delete an item

## Uninstall Doka

Run the installer again IN ADMINISTRATOR MODE

````bash
doka-one-installer.exe uninstall --installation-path "D:/app/doka.one" 
````

---

### Linux
