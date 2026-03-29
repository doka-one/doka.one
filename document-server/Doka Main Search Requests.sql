### REF_TAG : DOKA_SEARCH_SQL :  Real query model 

 ENV : cs_test_03  db : cs_93f71785  

-- Solution 1
SELECT i.id, name, file_ref, created_gmt, last_modified_gmt, ot.value_string my_email, ot2.value_string tag2
                    FROM cs_93f71785.item i ,
                    

                    (
                        select tv.item_id , tv.value_string  from tag_definition td inner join tag_value tv  on (tv.tag_id = td.id)
    					and td."name" =  'my_email'
    				) ot,					
    				(
                        select tv.item_id , tv.value_string  from tag_definition td inner join tag_value tv  on (tv.tag_id = td.id)
    					and td."name" =  'tag2'
    				) ot2
    				
    				where ot.item_id = i.id  and ot2.item_id = i.id					
    				and EXISTS  (
    					select 1  from tag_definition td inner join tag_value tv  on (tv.tag_id = td.id)
    					and td."name" =  'my_email' and tv.value_string like 't%' and tv.item_id = i.id
    					union 
    					select 1  from tag_definition td inner join tag_value tv  on (tv.tag_id = td.id)
    					and td."name" =  'tag2' and tv.value_string like 'bla%' and tv.item_id = i.id
    				)					
    				order by ot.value_string

   

-- 1 mettre plus de données dans la DB
-- 2 faire une réquete ou l'on puisse décider l'organisation des conditions (ET) et (OU)					
					
                    --INNER JOIN   cs_0e542da4.tag_value tv on (tv.item_id = i.id)
                    -- WHERE  ({1})
                   --  ORDER BY prop.col1 


​                    
select tv.value_string  from tag_definition td inner join tag_value tv  on (tv.tag_id = td.id)
and td."name" =  'my_email'
where tv.item_id = 1

select * from tag_definition td inner join tag_value tv  on (tv.tag_id = td.id)


--- Solution 2

-- (active == true and ( my_email like %inc.com or title LIKE 'ba%' ))

SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_my_email.value AS my_email,
       ot_title.value AS title,
       ot_active.value as active,
       ot_birthdate.value as birthdate
FROM item i
-- subquery
LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_value tv WHERE tv.tag_id = 1  AND  unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%inc.com')
) ot_my_email ON ot_my_email.item_id = i.id
-- subquery
LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'title' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ba%')
) ot_title ON ot_title.item_id = i.id
-- subquery
LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_boolean as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true  
    -- super filter
    and tv.item_id in ( 
			    SELECT tv.item_id
			    FROM tag_definition td
			    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'my_email' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%inc.com')
			    union 
			    SELECT tv.item_id
			    FROM tag_definition td
			    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'title' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ba%')
    )
) ot_active ON ot_active.item_id = i.id
-- subquery
LEFT OUTER join (
    SELECT tv.item_id, tv.value_date as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'birthdate'
) ot_birthdate ON ot_birthdate.item_id = i.id
WHERE ot_active.value = true and ( ot_my_email.value is not null or ot_title.value is not null )  -- composable filter
ORDER BY ot_my_email.value  offset 0 limit 10; 

ALGO _ 

** Parse the input conditions in an AST; implement the "AND" precedence over "OR" **
** Identify the indexed conditions (IC) and not indexed conditions (NIC) 
** search for the IC we can inject through an "AND"
			Terminal leaf of the AST like C D  and E F Z  in the "( A or B ) and ( C and D or (E and F) and Z )"
			IC injection can apply to (E,F,Z) because we detected the pattern (E and F) and Z in the AST
			for that purpose, it s could be interesting to group the terminal AND conditions in the AST (the tree is no longer binary then)
** create a subquery for each condition with injected super filters 
** add the composable filter to the main query **
** generate the final PG sql **

Note : 
	1. There is the IC/NIC concept but also the Filtering or not filtering conditions and the Injection into the ANDs can help
	2. The fact we plan to stored the Doka Queries could allow us to estimate the NIC/NF of each conditions and then to pre-compute the SQL query.
	3. This algo gives an efficient solution to retreive the fields explictly mentionned in the filter or the order (TODO),
		but it does not bring all the tag values of each items (A second query to show all the tag values for a list of item id could be ok) 


Question : Can we manage complex conditions like ( A or B ) and ( C and D or (E and F) ) ---> Yes, si below

    SELECT count(*)
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' AND tv.value_integer = 30099 OR tv.value_integer = 30098



select * from tag_definition

    SELECT count(*) as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'my_email' AND tv.value_string LIKE '%inc.com'
    
    SELECT count(*)
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true


​    
    SELECT count(*)
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'birthdate' 

-- Création de l'index sur la colonne "name" de la table "tag_definition"
CREATE INDEX idx_tag_definition_name ON tag_definition ("name");

-- Création de l'index sur les colonnes "tag_id" et "value_string" de la table "tag_value"
CREATE INDEX idx_tag_value_tag_id_value_string ON tag_value (tag_id, value_string);

    SELECT tv.item_id, tv.value_string as value
    FROM  tag_value tv where tv.tag_id = (select id
    FROM tag_definition td
    where  td.name = 'my_email' )  AND tv.value_string LIKE 'me@inc.com'
    
    select id
    FROM tag_definition td
    where  td.name = 'title' 
    
    select tv.item_id, tv.value_string as value
    FROM tag_value tv WHERE tv.tag_id = 1  AND  tv.value_string like 'me@inc.%'
    
    SELECT *
    	FROM tag_value
    	WHERE unaccent_lower((value_string)::text) LIKE unaccent_lower('%inc.com');
    
    select tv.item_id, tv.value_string as value
    FROM tag_value tv WHERE tv.value_string like 'me@inc.%'
    
    select tv.item_id, tv.value_string as value
    FROM tag_value tv WHERE tv.tag_id = 7  
    AND unaccent_lower((value_string)::text) LIKE unaccent_lower('ba%');
    
       SELECT *
    	FROM tag_value
    	WHERE unaccent_lower((value_string)::text) LIKE unaccent_lower('ba%');

    
---------------------​    
-- CASE 1 (active == true AND postal_code = 30500)
---------------------

    SELECT count(*) as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' AND tv.value_integer = 30099


​    
    SELECT count(*) as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' AND tv.value_boolean = true


​    
 SELECT i.id,
​       name,
​       file_ref,
​       created_gmt,
​       last_modified_gmt,
​       ot_lastname.value AS lastname,
​       ot_postal_code.value AS postal_code,
​       ot_active.value as active
FROM item i

left outer JOIN (
​    SELECT tv.item_id, tv.value_string as value
​    FROM tag_definition td
​    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' /*AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('abb%')   */
) ot_lastname ON ot_lastname.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' AND tv.value_integer = 30099
) ot_postal_code ON ot_postal_code.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_boolean as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true      
) ot_active ON ot_active.item_id = i.id

where ot_active.value is not null  and  ot_postal_code.value is not null
ORDER BY ot_lastname.value  offset 0 limit 10; 
    
---------------------    
-- CASE 2 (active == true AND lastname LIKE 'ao%')
---------------------
    
    SELECT count(*) as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ao%');

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value AS lastname,
       ot_postal_code.value AS postal_code,
       ot_active.value as active
FROM item i
left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ao%')
) ot_lastname ON ot_lastname.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code'
) ot_postal_code ON ot_postal_code.item_id = i.id
left outer JOIN (
    SELECT tv.item_id, tv.value_boolean as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true      
) ot_active ON ot_active.item_id = i.id
where ot_active.value is not null  and ot_lastname.value is not null
ORDER BY ot_lastname.value  offset 0 limit 10;    

---------------------
-- CASE 3 (active == true AND lastname LIKE 'a%')
---------------------
    
    SELECT count(*) as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('a%');

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value AS lastname,
       ot_postal_code.value AS postal_code,
       ot_active.value as active
FROM item i
left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('a%')
    																		and tv.item_id in (
    																		    SELECT tv.item_id
																						    FROM tag_definition td
																						    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true    
    																		)  
) ot_lastname ON ot_lastname.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code'
) ot_postal_code ON ot_postal_code.item_id = i.id
left outer JOIN (
    SELECT tv.item_id, tv.value_boolean as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true      
) ot_active ON ot_active.item_id = i.id
where ot_active.value is not null  and ot_lastname.value is not null
ORDER BY ot_lastname.value  offset 10 limit 10;  

---------------------
-- CASE 4 lastname LIKE 'ab%'
---------------------

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value AS lastname,
       ot_postal_code.value AS postal_code,
       ot_active.value as active
FROM item i
left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
) ot_lastname ON ot_lastname.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code'
) ot_postal_code ON ot_postal_code.item_id = i.id
left outer JOIN (
    SELECT tv.item_id, tv.value_boolean as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' 
) ot_active ON ot_active.item_id = i.id
where ot_lastname.value is not null 
ORDER BY ot_lastname.value  offset 0 limit 20; 

---------------------
-- CASE 5 lastname LIKE 'h%' and postal_code = 30099
---------------------

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value AS lastname,     
       ot_postal_code.value AS postal_code,
       ot_active.value as active
FROM item i
left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('h%')
) ot_lastname ON ot_lastname.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099
) ot_postal_code ON ot_postal_code.item_id = i.id
left outer JOIN (
    SELECT tv.item_id, tv.value_boolean as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' 
) ot_active ON ot_active.item_id = i.id
where ot_lastname.value is not null and ot_postal_code.value is not null
ORDER BY ot_lastname.value  offset 0 limit 50; 


---------------------
-- CASE 6 lastname LIKE 'ab%' OR  (postal_code = 30099 AND lastname LIKE 'h%')
---------------------

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       COALESCE(ot_lastname.value, ot_lastname_2.value)   AS lastname3,
       ot_lastname.value AS lastname,
       ot_lastname_2.value as lastname2,
       ot_postal_code.value AS postal_code
FROM item i

left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
) ot_lastname ON ot_lastname.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099
) ot_postal_code ON ot_postal_code.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('h%')
      /* and tv.item_id in (
       		    SELECT tv.item_id
			    FROM tag_definition td
			    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099       
       ) */ -- super filtre 0.7 --> 0.2
) ot_lastname_2 ON ot_lastname_2.item_id = i.id

where ot_lastname.value is not null or ( ot_lastname_2.value is not null and ot_postal_code.value is not null )
ORDER BY COALESCE(ot_lastname.value, ot_lastname_2.value)  DESC offset 400 limit 50; 

---------------------
-- CASE 7 lastname LIKE "ab%" OR  (postal_code == 30099 AND lastname LIKE "%h%")
---------------------

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       COALESCE(ot_lastname.value, ot_lastname_2.value) AS lastname3,
       ot_lastname.value AS lastname,
       ot_lastname_2.value as lastname2,
       ot_postal_code.value AS postal_code
FROM cs_93f71785.item i

LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM cs_93f71785.tag_definition td
    JOIN cs_93f71785.tag_value tv ON 
    	tv.tag_id = td.id 
    	AND td."name" = 'lastname' 
    	AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')    	
) ot_lastname ON ot_lastname.item_id = i.id

LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM cs_93f71785.tag_definition td
    JOIN cs_93f71785.tag_value tv ON 
    	tv.tag_id = td.id 
    	AND td."name" = 'postal_code' 
    	AND tv.value_integer = 30099
    	AND tv.item_id IN (
	    	SELECT tv.item_id
	    		FROM cs_93f71785.tag_definition td
	    		JOIN cs_93f71785.tag_value tv ON 
	    			tv.tag_id = td.id 
	    			AND td."name" = 'lastname' 
	    			AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')    	
    	)
) ot_postal_code ON ot_postal_code.item_id = i.id

LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM cs_93f71785.tag_definition td
    JOIN cs_93f71785.tag_value tv ON 
    	tv.tag_id = td.id 
    	AND td."name" = 'lastname' 
    	AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')
        AND tv.item_id IN (
       		    SELECT tv.item_id
			    FROM cs_93f71785.tag_definition td
			    JOIN cs_93f71785.tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' AND tv.value_integer = 30099       
       ) -- super filtre 4s --> 0.2
) ot_lastname_2 ON ot_lastname_2.item_id = i.id

WHERE 
	ot_lastname.value IS NOT NULL OR ( ot_lastname_2.value IS NOT NULL AND ot_postal_code.value IS NOT NULL )
ORDER BY 
	COALESCE(ot_lastname.value, ot_lastname_2.value)  DESC 
OFFSET 400 LIMIT 500; 

--- CASE 7 : simplifié pour tests Rust

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       COALESCE(ot_lastname.value, ot_lastname_2.value) AS lastname3,
       ot_lastname.value AS lastname,
       ot_lastname_2.value as lastname2,
       ot_postal_code.value AS postal_code
FROM item i
LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON 
    	tv.tag_id = td.id 
    	AND td."name" = 'lastname' 
    	AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')    	
) ot_lastname ON ot_lastname.item_id = i.id
LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON 
    	tv.tag_id = td.id 
    	AND td."name" = 'postal_code' 
    	AND tv.value_integer = 30099    	
) ot_postal_code ON ot_postal_code.item_id = i.id
LEFT OUTER JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON 
    	tv.tag_id = td.id 
    	AND td."name" = 'lastname' 
    	AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')
) ot_lastname_2 ON ot_lastname_2.item_id = i.id
WHERE 
	ot_lastname.value IS NOT NULL OR ( ot_lastname_2.value IS NOT NULL AND ot_postal_code.value IS NOT NULL )
ORDER BY 
	COALESCE(ot_lastname.value, ot_lastname_2.value)  DESC 
OFFSET 400 LIMIT 50; 



---------------------
-- CASE 8 lastname LIKE 'ab%' AND  (postal_code = 30099 OR lastname LIKE '%h%') => (lastname LIKE 'ab%' AND postal_code = 30099) OR (lastname LIKE 'ab%' AND lastname LIKE '%h%') : 0.366 s 
---------------------

    SELECT count(*) as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%');
    
    SELECT count(*) as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%');
       
       SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%') AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')     
       and tv.item_id in (
       		    SELECT tv.item_id
        		FROM tag_definition td
        		JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')      
       ) -- super filtre 4s --> 0.2 

-- RG : if the conditions is not indexed, it's not productive to inject it as a super filter

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       coalesce ( COALESCE(ot_lastname_ab.value, ot_lastname_h.value), ot_lastname_ab_2.value)   AS lastname,
       ot_lastname_ab.value AS lastname_ab,
       ot_lastname_ab_2.value AS lastname_ab_2,
       ot_lastname_h.value as lastname_h,
       ot_postal_code.value AS postal_code
FROM item i

left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
           and tv.item_id in (
       		    SELECT tv.item_id
			    FROM tag_definition td
			    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099       
       ) -- super filtre 4s --> 0.2        
) ot_lastname_ab ON ot_lastname_ab.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
    /*and tv.item_id in (
	    	SELECT tv.item_id
	    		FROM tag_definition td
	    		JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')    	
    	)*/
) ot_lastname_ab_2 ON ot_lastname_ab_2.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099
    	and tv.item_id in (
	    	SELECT tv.item_id
	    		FROM tag_definition td
	    		JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')    	
    	)
) ot_postal_code ON ot_postal_code.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')
       and tv.item_id in (
       		    SELECT tv.item_id
	    		FROM tag_definition td
	    		JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')      
       ) -- super filtre 4s --> 0.2 
) ot_lastname_h ON ot_lastname_h.item_id = i.id

where (ot_lastname_ab.value is not null and ot_postal_code.value is not null) or ( ot_lastname_h.value is not null and ot_lastname_ab_2.value is not null)
ORDER BY coalesce ( COALESCE(ot_lastname_ab.value, ot_lastname_h.value), ot_lastname_ab_2.value)   DESC offset 400 limit 10; 

-- Same with no simplification  lastname LIKE 'ab%' AND  (postal_code = 30099 OR lastname LIKE '%h%') : 0.038 s

 SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       COALESCE(ot_lastname_ab.value, ot_lastname_h.value)   AS lastname,
       ot_lastname_ab.value AS lastname_ab,      
       ot_lastname_h.value as lastname_h,
       ot_postal_code.value AS postal_code
FROM item i

left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')     
) ot_lastname_ab ON ot_lastname_ab.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_integer as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099
       and tv.item_id in (
       		    SELECT tv.item_id
	    		FROM tag_definition td
	    		JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')      
       ) -- super filtre 4s --> 0.2 
) ot_postal_code ON ot_postal_code.item_id = i.id

left outer JOIN (
    SELECT tv.item_id, tv.value_string as value
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')
       and tv.item_id in (
       		    SELECT tv.item_id
	    		FROM tag_definition td
	    		JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')      
       ) -- super filtre 4s --> 0.2 
) ot_lastname_h ON ot_lastname_h.item_id = i.id

-- lastname LIKE 'ab%' AND  (postal_code = 30099 OR lastname LIKE '%h%') 
where (ot_lastname_ab.value is not null) and (ot_lastname_h.value is not null or ot_postal_code.value is not null)
ORDER BY COALESCE(ot_lastname_ab.value, ot_lastname_h.value)  DESC offset 400 limit 10;

---
---
---

doka-cli item create -n ripoux -p "(my_email:me@inc.com)(title:ripoux)(active)"
doka-cli item create -n avenger -p "(my_email:me@inc.com)(title:avenger)(active:false)"
doka-cli item create -n spiderman -p "(my_email:me@inc.com)(title:spiderman)(active)"
doka-cli item create -n bladerunner -p "(my_email:me@inc.com)(title:bladerunner)(active)"
doka-cli item create -n starwars -p "(my_email:me@inc.com)(title:starwars)(active)"
doka-cli item create -n back_to_the_future -p "(my_email:me@inc.com)(title:back to the future)(active)"

VACUUM ANALYZE item;
VACUUM ANALYZE tag_definition;
VACUUM ANALYZE tag_value;

CALL create_items_bulk(
    14000
);

select count(*) from item
select count(*) from tag_value

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

