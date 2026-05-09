### Implement OAuth2

> Note : This section need more work to define the precise phases of OAuth2 implementation for Doka

We can implement the OAuth2 is a few phases 
1. Create a sysadmin user by default , never linked to any customer nor database. The sysadmin password 
  can be stored in an cyphered file and read when the service starts.
2. Implement JWT for the session token for all the standard users
3. The login with the sysadmin must generate a JWT token too
4. The machine to machine connection to access "token" API must be donne in 2 steps through a specific client_id / client_secret
