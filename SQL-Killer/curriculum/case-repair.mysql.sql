-- MySQL / InnoDB. Run only in an empty SQL-Killer-owned teaching sandbox.
-- The case's automatic verifier creates its own isolated database instead.
CREATE TABLE IF NOT EXISTS CaseCourse(
  Cno VARCHAR(255) PRIMARY KEY, capacity INT NOT NULL, enrolled_count INT NOT NULL
) ENGINE=InnoDB;
CREATE TABLE IF NOT EXISTS Enrollment(
  request_id VARCHAR(255) PRIMARY KEY, Sno INT NOT NULL,
  Cno VARCHAR(255) NOT NULL, status VARCHAR(20) NOT NULL, UNIQUE(Sno,Cno)
) ENGINE=InnoDB;

DROP PROCEDURE IF EXISTS attempt_enroll_locked;
DROP PROCEDURE IF EXISTS attempt_enroll_atomic;
DELIMITER //
CREATE PROCEDURE attempt_enroll_locked(IN p_cno VARCHAR(255), IN p_sno INT, IN p_request VARCHAR(255))
BEGIN
  DECLARE current_count INT;
  DECLARE max_count INT;
  DECLARE EXIT HANDLER FOR SQLEXCEPTION BEGIN ROLLBACK; RESIGNAL; END;
  START TRANSACTION;
  SELECT enrolled_count,capacity INTO current_count,max_count
    FROM CaseCourse WHERE Cno=p_cno FOR UPDATE;
  IF current_count < max_count THEN
    UPDATE CaseCourse SET enrolled_count=enrolled_count+1 WHERE Cno=p_cno;
    INSERT INTO Enrollment VALUES(p_request,p_sno,p_cno,'success');
    COMMIT;
    SELECT 'accepted' AS result;
  ELSE
    ROLLBACK;
    SELECT 'full' AS result;
  END IF;
END//

CREATE PROCEDURE attempt_enroll_atomic(IN p_cno VARCHAR(255), IN p_sno INT, IN p_request VARCHAR(255))
BEGIN
  DECLARE EXIT HANDLER FOR SQLEXCEPTION BEGIN ROLLBACK; RESIGNAL; END;
  START TRANSACTION;
  UPDATE CaseCourse SET enrolled_count=enrolled_count+1
    WHERE Cno=p_cno AND enrolled_count<capacity;
  IF ROW_COUNT()=1 THEN
    INSERT INTO Enrollment VALUES(p_request,p_sno,p_cno,'success');
    COMMIT;
    SELECT 'accepted' AS result;
  ELSE
    ROLLBACK;
    SELECT 'full' AS result;
  END IF;
END//
DELIMITER ;
-- With a freshly initialized course, run different requests in sessions A and B:
-- INSERT INTO CaseCourse VALUES('C1',40,0);
-- CALL attempt_enroll_locked('C1',100,'request-A');
-- CALL attempt_enroll_locked('C1',101,'request-B');
-- Or use attempt_enroll_atomic for both requests.
-- Compare COUNT(*) for successful enrollments with the course counter.
-- Insertion failures roll back the reservation too; duplicate requests cannot
-- consume another seat. Every writer must follow the same reservation protocol.
