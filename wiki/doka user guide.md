## ~~Doka User Guide~~

##### ~~Create a customer~~ 

~~Open a new windows command line (to enjoy the newly created DOKA_CLI_ENV), environment variable)~~

````bash
doka-cli token generate -c %DOKA_ENV%\document-server\keys\cek.key
````

---


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

### Search items

```bash
doka-cli item search
    -f "(lastname LIKE \"a%\") OR (postal_code == 34500 AND lastname LIKE \"%h%\")"
    -s "lastname desc"
```

The filter follows the Doka Filter Syntax (DFS). Rules:

- Conditions are `attribute <op> value` with `<op>` ∈ `== != < <= > >= LIKE`.
  Use `==`, not `=`. A single `=` is rejected.
- String values are always double-quoted. Numbers and `TRUE` / `FALSE` are not.
- Inside a quoted string, the characters `"`, `%` and `#` must be escaped with
  a leading `#`: `#"` → `"`, `#%` → `%`, `##` → `#`.
- In a `LIKE` value, a bare `%` is the wildcard (any sequence of characters).
  Use `#%` for a literal percent. A bare `%` outside a `LIKE` value, or any
  unescaped `#` / `"` / `%`, is a syntax error (`InvalidEscapeSequence`).
- Combine with `AND` / `OR` (case-insensitive) and group with `( ... )`.
  `AND` binds tighter than `OR`; use parentheses to override.

Example combining escapes:

```bash
doka-cli item search -f "(category == \"super #\"extra#\" player\") AND (note == \"see paragraph ##6\")"
```

The parser receives:

```
(category == "super #"extra#" player") AND (note == "see paragraph ##6")
```

and stores `super "extra" player` and `see paragraph #6` as the two values.

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
