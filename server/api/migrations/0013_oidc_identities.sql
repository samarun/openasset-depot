-- Link local users to an external identity provider.
--
-- SSO users are still ordinary rows in `users`, so every existing permission,
-- lock, and audit query keeps working. What changes is how they authenticate:
-- an OIDC subject is matched instead of a password hash.

-- Issuer plus subject is the only pair an OIDC provider guarantees to be stable.
-- Email and username can both be reassigned, so neither is used as the identity.
ALTER TABLE users ADD COLUMN oidc_issuer TEXT;
ALTER TABLE users ADD COLUMN oidc_subject TEXT;

CREATE UNIQUE INDEX idx_users_oidc_identity
    ON users (oidc_issuer, oidc_subject)
    WHERE oidc_subject IS NOT NULL;

-- Password login is disabled for provisioned SSO accounts: a directory-managed
-- user must not keep a second, locally-managed way in.
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;

ALTER TABLE users ADD CONSTRAINT users_has_credential CHECK (
    password_hash IS NOT NULL OR oidc_subject IS NOT NULL
);
