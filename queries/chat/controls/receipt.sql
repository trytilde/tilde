SELECT EXISTS(SELECT 1 FROM invocation_control_receipts WHERE invocation_id=$1 AND command_id=$2) AS "exists!";
