ALTER TABLE taxpayers
    ADD COLUMN password_hash TEXT;

UPDATE taxpayers
SET password_hash = '$argon2id$v=19$m=19456,t=2,p=1$c3RjLWRlbW8tc2FsdA$lqBv1+dGY/YA6smKL5ZQf+KbijO6qJEm4RtWqFvZi4c'
WHERE password_hash IS NULL;

ALTER TABLE taxpayers
    ALTER COLUMN password_hash SET NOT NULL;
