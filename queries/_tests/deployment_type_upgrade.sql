-- Old agent-wide sidecar mode can coexist with a newer Lambda record that cannot serve yet.
INSERT INTO agents(id,name,endpoint_url,webhook_signing_key,deployment_mode)
VALUES('00000000-0000-4000-8000-000000000071','Old Sidecar',NULL,decode(repeat('00',46),'hex'),'sidecar');
INSERT INTO agent_deployments(id,agent_id,source,target,target_reference,created_at)
VALUES('00000000-0000-4000-8000-000000000072','00000000-0000-4000-8000-000000000071','ci','sidecar',NULL,NOW()-INTERVAL '1 day'),
('00000000-0000-4000-8000-000000000073','00000000-0000-4000-8000-000000000071','ci','aws_lambda','arn:aws:lambda:eu-central-1:123456789012:function:next',NOW());
UPDATE agent_deployment_settings SET serving_deployment_id='00000000-0000-4000-8000-000000000072' WHERE agent_id='00000000-0000-4000-8000-000000000071';
UPDATE agent_deployments SET traffic_weight=100 WHERE id='00000000-0000-4000-8000-000000000072';
INSERT INTO chat_threads(id,title,primary_agent_id) VALUES('00000000-0000-4000-8000-000000000074','Existing conversation','00000000-0000-4000-8000-000000000071');
INSERT INTO chat_participants(id,thread_id,agent_id) VALUES('00000000-0000-4000-8000-000000000075','00000000-0000-4000-8000-000000000074','00000000-0000-4000-8000-000000000071');
