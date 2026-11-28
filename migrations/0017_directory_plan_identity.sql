-- Separate the immutable cleanup tree from the original ownership evidence.
ALTER TABLE removal_plan_items ADD COLUMN authority_identity TEXT;
UPDATE removal_plan_items SET authority_identity=expected_identity;
UPDATE removal_plans SET state='INVALIDATED',revision=revision+1 WHERE state IN ('DRAFT','APPROVED');
