SELECT EXISTS(SELECT 1 FROM agents WHERE id=$1) AS "exists!";
