### Install procedure

#### Windows

Install Postgresql 11 or +

* Get a PG user with admin privileges (Ex : user: john, password : doo)

Download the doka_one_installer.exe (SHA-256 : ........... )

Run the doka_one_installer.exe

> Enter the installation path :  e:\doka.one
> Enter Postgresql host name : localhost
> Enter Postgresql user name : john
> Enter Postgresql user password : doo
> Enter Instance name (default: prod_1) :
>
> ....Installation path verified ✔
> ....Postgresql connection verified ✔
> ....Download apache Tika server ... ✔ ... Found port 40'010 available
> ....Download key-manager ... ✔ ... Found port 30'040 available
> ....Download session-manager ... ✔ ... Foudn port 30'050 available
> ....Download admin-server ... ✔ ... Found port 30'060 available
> ....Download document-server ... ✔ ... Found port 30'070 available
> ....Download file-server ... ✔ ... Found port 30'080 available
> ....Download doka-cli ... ✔
>
> Initialize administration schema (ad_prod_1) ... ✔
>
> Generate master key ... ✔
>
> Building configuration files for micro-services ... ✔
>
> Building windows services ... ✔
>
> Apache Tika Server is running on port : 40'010 (see logs in e:\doka.one\logs\\tika.log)
> key-manager is running on port 30'040 (see logs in e:\doka.one\logs\key-manager.log)
> session-manager is running on port 30'050 (see logs in e:\doka.one\logs\admin-server.log)
> admin-server is running on port 30'060
> document-server is running on port 30'070
> file-server is running on port 30'080

##### Start doka one

doka-cli doka start

##### Shutdown doka one

doka-cli doka shutdown

##### Create a customer

doka-cli customer create ...

##### Login

doka-cli session login -u ... -p ...

##### Create an item

List all the items

##### Download an attached file

Delete an item


---



#### Linux
