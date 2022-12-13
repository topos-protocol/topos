Feature: Topos API
    Background:
        Given subnet "pk0xba" setup
        And subnet "pk0x12" setup
        Given topos-node client 1 at "localhost" port 6000
        Given topos-node client 2 at "localhost" port 6001
        # topos-node client part of the Topos Subnet
        Given topos-subnet-node client 3 at "localhost" port 6002

    Scenario Outline: GET /health
        When I use <client> to check the service health
        Then I receive status code <status>

        Examples:
            | client | status |
            | 1      | 200    |

    #
    # Get the information related to the client
    #
    # Can replace the list of followed subnet by bloom filter to avoid eclipse attack
    #
    Scenario Outline: GET /info
        When I use <client> to get the "<info>"
        Then I receive the list of <subnet-id> that the <client> is following

        # Recall that the subnet id is the subnet frost pk
        Examples:
            | client | subnet-id                |
            | 1      | [pk0xba, pk0xef, pk0x12] |
    #
    # Get the information given on certificate
    #
    # COMMITTED : The certificate is created and signed by the subnet
    # SUBMITTED : The certificate is sent to the TCE for delivery
    # DELIVERED : The certificate is delivered by the TCE
    # COMPLETED : The certificate is included by all receiver subnet
    Scenario Outline: GET /certificates/<cert-hash>
        When I use <client> to fetch information for <cert-hash>
        Then I receive the "<status>", with the certificate contents, <txs> cross-chain transactions, "<prev_cert_id>" the previous cert hash

        Examples:
            | client | <cert-hash> | subnet-id | status           | txs             | prev_cert_id |
            | 1      | 0x11113     | 0xf00     | "UNKNOWN_SUBNET" |                 |              |
            | 1      | 0x20193     | pk0xba    | "COMMITED"       | [tx1, tx2, ...] | 0x222b3      |
            | 1      | 0x092b3     | pk0xc1    | "SUBMITTED"      | [tx1, tx2, ...] | 0x321b8      |
            | 1      | 0x122b3     | pk0xef    | "DELIVERED"      | [tx1, tx2, ...] | 0x331bc      |
            | 1      | 0x122b3     | pk0xef    | "COMPLETED"      | [tx1, tx2, ...] | 0x331bc      |
    #
    # Get the certificates given one subnet
    #
    # If no number is specified, return the whole list of cert from the given subnet
    Scenario Outline: GET /certificates/from/<subnet-id>/<number>
        When I use <client> to fetch certificate <number> from <subnet-id>
        Then the response contains <txs> cross-chain transactions, the previous cert hash "<prev_hash>"

        Examples:
            | client | subnet-id | number | status           | txs             | prev_hash |
            | 1      | 0xf00     | 24     | "UNKNOWN_SUBNET" |                 |           |
            | 1      | pk0xba    | 24     | "COMMITED"       | [tx1, tx2, ...] | 0x222b3   |
            | 1      | pk0xc1    | 24     | "SUBMITTED"      | [tx1, tx2, ...] | 0x321b8   |
            | 1      | pk0xef    | 24     | "DELIVERED"      | [tx1, tx2, ...] | 0x331bc   |

    #
    # Get the list of certificates received by <subnet-id>
    #
    Scenario Outline: GET /certificates/to/<subnet-id>/
        When I use <client> to fetch certificate sent to <subnet-id>
        Then I receive the
        Examples:
            | client | number | status      |
            | 1      | 24     | "COMMITED"  |
            | 1      | 24     | "SUBMITTED" |
            | 1      | 24     | "DELIVERED" |

    #
    # Get the list of registered subnet
    #
    # Recall that the subnet id is the frost pk
    Scenario Outline: GET /topos/subnet/all
        When I request the list of registered <client> to the Topos Subnet
        Then I receive the list of registered <subnet-id>

        Examples:
            | client | subnet-id                |
            | 3      | [pk0xba, pk0xef, pk0x12] |
