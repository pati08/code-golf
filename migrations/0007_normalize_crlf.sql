-- Normalize CRLF line endings to LF in stored code and fix byte counts.
-- LENGTH(CAST(... AS BLOB)) gives byte length (matches Rust's str::len).
-- RTRIM with char(10) trims only trailing newlines, matching trim_end_matches('\n').
UPDATE submissions
SET
    code = REPLACE(code, char(13) || char(10), char(10)),
    byte_count = LENGTH(CAST(
        RTRIM(REPLACE(code, char(13) || char(10), char(10)), char(10))
    AS BLOB))
WHERE instr(code, char(13)) > 0;
