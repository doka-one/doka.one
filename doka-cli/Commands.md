Only for admin user, we generate a token with the cek, it will allow the folllowing commands

doka-cli token generate -c C:\Users\denis\doka-configs\dev-one\document-server\keys\cek.key

Then we can create an admin customer

doka-cli customer create -n "DENIS_CUST" -e "denis.2@inc.com" -ap "Myadmin123;"
