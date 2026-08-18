UPDATE messages SET parent_message_id = -parent_message_id WHERE parent_message_id < 0;

UPDATE messages m
SET parent_message_id = sub.channel_message_id
FROM (
    SELECT m1.id AS target_id, m2.channel_message_id::bigint AS channel_message_id
    FROM messages m1
    JOIN messages m2 ON m1.parent_message_id = m2.id
    WHERE m1.parent_message_id > 0 AND m2.channel_message_id IS NOT NULL
) sub
WHERE m.id = sub.target_id;

UPDATE messages SET status = 'pending' WHERE status = 'publishing';
