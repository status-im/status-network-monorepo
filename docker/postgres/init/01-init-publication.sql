-- Enable logical replication for the required tables
CREATE PUBLICATION rln_publication FOR TABLE tx_counter, deny_list;
