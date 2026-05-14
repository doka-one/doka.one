# Doka Main Search Requests Bulk Data

## CLI Data Setup

```bash
doka-cli item create -n ripoux -p "(my_email:me@inc.com)(title:ripoux)(active)"
doka-cli item create -n avenger -p "(my_email:me@inc.com)(title:avenger)(active:false)"
doka-cli item create -n spiderman -p "(my_email:me@inc.com)(title:spiderman)(active)"
doka-cli item create -n bladerunner -p "(my_email:me@inc.com)(title:bladerunner)(active)"
doka-cli item create -n starwars -p "(my_email:me@inc.com)(title:starwars)(active)"
doka-cli item create -n back_to_the_future -p "(my_email:me@inc.com)(title:back to the future)(active)"
```

## Maintenance Queries

```sql
VACUUM ANALYZE item;
VACUUM ANALYZE tag_definition;
VACUUM ANALYZE tag_value;

CALL create_items_bulk(
    14000
);

select count(*) from item;
select count(*) from tag_value;
```

## Bulk Generator Procedure

```sql
CREATE OR REPLACE PROCEDURE create_items_bulk(IN num_items INT)
LANGUAGE plpgsql
AS $$
DECLARE
    i INT;
    item_id BIGINT;
    postal_code_tag_id BIGINT;
    lastname_tag_id BIGINT;
    firstname_tag_id BIGINT;
    active_tag_id BIGINT;
BEGIN
    -- Récupérer les ID des tags nécessaires
    SELECT id INTO postal_code_tag_id FROM tag_definition WHERE "name" = 'postal_code';
    SELECT id INTO lastname_tag_id FROM tag_definition WHERE "name" = 'lastname';
    SELECT id INTO firstname_tag_id FROM tag_definition WHERE "name" = 'firstname';
    SELECT id INTO active_tag_id FROM tag_definition WHERE "name" = 'active';

    -- Boucle pour créer les items
    FOR i IN 1..num_items LOOP
        -- Créer l'item
        INSERT INTO item ("name", created_gmt, last_modified_gmt)
        VALUES ('item_'|| i, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        RETURNING id INTO item_id;

        -- Associer les tags à l'item
        INSERT INTO tag_value (tag_id, item_id, value_integer)
        VALUES (postal_code_tag_id, item_id, generer_nombre_aleatoire());

        INSERT INTO tag_value (tag_id, item_id, value_string)
        VALUES (lastname_tag_id, item_id,  generer_chaine_aleatoire());

        INSERT INTO tag_value (tag_id, item_id, value_string)
        VALUES (firstname_tag_id, item_id,  generer_chaine_aleatoire());

        INSERT INTO tag_value (tag_id, item_id, value_boolean)
        VALUES (active_tag_id, item_id, retourner_vrai_une_fois_sur_mille());

       IF (i % 1000 = 0) THEN
          commit;
       end if;
    END LOOP;
   commit;
END;
$$;
```

## Helper Functions

```sql
CREATE OR REPLACE FUNCTION generer_chaine_aleatoire()
RETURNS VARCHAR AS $$
DECLARE
    i INT := 0;
    resultat VARCHAR := '';
BEGIN
    FOR i IN 1..10 LOOP
        resultat := resultat || CHR(97 + floor(random() * 26)::int);
    END LOOP;

    RETURN resultat;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION generer_nombre_aleatoire()
RETURNS INTEGER AS $$
DECLARE
    nombre INTEGER;
BEGIN
    -- Générer un nombre aléatoire entre 10000 et 99999
    nombre := floor(random() * 90000)::int + 10000;

    RETURN nombre;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION retourner_vrai_une_fois_sur_mille()
RETURNS BOOLEAN AS $$
DECLARE
    proba NUMERIC;
BEGIN
    proba := random();
    RETURN proba < 0.001;
END;
$$ LANGUAGE plpgsql;

SELECT retourner_vrai_une_fois_sur_mille();
SELECT generer_nombre_aleatoire();
SELECT generer_chaine_aleatoire();
```
