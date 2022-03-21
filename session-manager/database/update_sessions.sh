#! /bin/bash
#
# 0 */2 * * *  $HOME/doka_one_scheduled/update_sessions.sh
#
psql -d p2_prod_2 -U denis << SQL
UPDATE dokasys.sessions
        SET termination_time_gmt = ( NOW() at time zone 'UTC' )
WHERE
        termination_time_gmt IS NULL AND
        EXTRACT(EPOCH FROM (COALESCE(renew_time_gmt, start_time_gmt) + (120 * 60 * interval '1 second')
        - ( NOW() at time zone 'UTC'  ))) <= 0
SQL
