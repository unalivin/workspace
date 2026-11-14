INSERT INTO t (a) VALUES ('-- text'), ('it''s /* fine */');
SELECT "odd--name" FROM t WHERE a = 1 - 2;
