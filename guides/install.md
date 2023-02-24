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

> Enter the installation path :  e:\doka.one
> Enter Postgresql host name : localhost
> Enter Postgresql user name : john
> Enter Postgresql user password : doo
> Enter Instance name (default: prod_1) :

Phase 2: Verification 

> ....Installation path verified ✔
> ....Postgresql connection verified ✔

Phase 3: Download artefacts

> ....Download apache Tika server ... ✔ ... Found port 40'010 available
> ....Download key-manager ... ✔ ... Found port 30'040 available
> ....Download session-manager ... ✔ ... Found port 30'050 available
> ....Download admin-server ... ✔ ... Found port 30'060 available
> ....Download document-server ... ✔ ... Found port 30'070 available
> ....Download file-server ... ✔ ... Found port 30'080 available
> ....Download doka-cli ... ✔

Phase 4: Initialization

> Initialize administration schema (ad_prod_1) ... ✔
> 
> Generate master key ... ✔
> 
> Building configuration files for micro-services ... ✔
>
> Building windows services ... ✔

Phase 5: Start up services

> Apache Tika Server is running on port : 40'010 (see logs in e:\doka.one\logs\\tika.log) <br>
> key-manager is running on port 30'040 (see logs in e:\doka.one\logs\key-manager.log) <br>
> session-manager is running on port 30'050 (see logs in e:\doka.one\logs\admin-server.log) <br>
> admin-server is running on port 30'060  <br>
> document-server is running on port 30'070 <br>
>file-server is running on port 30'080  <br>

##### Start doka one

doka-cli doka start

##### Shutdown doka one

doka-cli doka shutdown

##### Create a customer

Open a new windows command line (to enjoy the newly created DOKA_CLI_ENV), environment variable)

doka-cli customer create ...

##### Login

doka-cli session login -u ... -p ...

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
