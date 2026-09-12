SELECT EXISTS(SELECT 1 FROM telemetry_delivery WHERE id=$1) AS "exists!";
