CREATE TABLE contacts (
    id INTEGER PRIMARY KEY,
    phone_number TEXT,
    email TEXT,
    linked_id INTEGER REFERENCES contacts,
    link_precedence TEXT CHECK (link_precedence IN ('secondary', 'primary')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,

    -- secondary contacts must have a linked_id
    CONSTRAINT linked_id_not_null_when_secondary CHECK (link_precedence <> 'secondary' OR linked_id IS NOT NULL),

    -- unique constraint for email, phone_number combination
    CONSTRAINT unique_email_phone UNIQUE (email, phone_number)
);
