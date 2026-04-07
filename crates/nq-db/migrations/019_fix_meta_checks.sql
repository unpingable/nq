-- Migration 019: Fix meta-check queries to exclude meta findings.
-- Prevents recursive supervisory aliasing.

UPDATE saved_queries
SET sql_text = 'SELECT severity, domain, kind, host, subject, consecutive_gens FROM warning_state WHERE consecutive_gens > 60 AND finding_class = ''signal'' ORDER BY consecutive_gens DESC'
WHERE name = 'long-lived warnings';

-- Also fix critical_findings if it exists (was created manually)
UPDATE saved_queries
SET sql_text = 'SELECT severity, kind, host, message FROM warning_state WHERE severity = ''critical'' AND finding_class = ''signal'''
WHERE name = 'critical findings';
