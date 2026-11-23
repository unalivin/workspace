package service

import (
 "crypto/sha256"
 "database/sql"
 "encoding/hex"
 "fmt"
 "strings"
)

type Backup struct { ID int64 `json:"id"`; DeviceID int64 `json:"device_id"`; SHA256 string `json:"sha256"`; CreatedAt string `json:"created_at"`; Bytes int `json:"bytes"` }

// VerifyBackup checks stored bytes against the immutable digest before they are used for rollback.
func (s *Service) VerifyBackup(id int64) (bool,error) {
 if id<=0{return false,fmt.Errorf("backup id must be positive")}
 var text,want string
 if err:=s.db.Conn.QueryRow("SELECT config_text,sha256 FROM backups WHERE id=?",id).Scan(&text,&want);err!=nil{return false,err}
 got:=sha256.Sum256([]byte(text));return hex.EncodeToString(got[:])==strings.ToLower(want),nil
}
func (s *Service) BackupText(id int64) (string,error) {
 if id<=0{return "",fmt.Errorf("backup id must be positive")}
 var text string;err:=s.db.Conn.QueryRow("SELECT config_text FROM backups WHERE id=?",id).Scan(&text);return text,err
}
func (s *Service) ListBackups(deviceID int64,limit int)([]Backup,error){
 if limit<=0||limit>500{return nil,fmt.Errorf("limit must be between 1 and 500")}
 rows,err:=s.db.Conn.Query("SELECT id,device_id,sha256,created_at,length(config_text) FROM backups WHERE (?=0 OR device_id=?) ORDER BY id DESC LIMIT ?",deviceID,deviceID,limit);if err!=nil{return nil,err};defer rows.Close()
 out:=[]Backup{};for rows.Next(){var b Backup;if err:=rows.Scan(&b.ID,&b.DeviceID,&b.SHA256,&b.CreatedAt,&b.Bytes);err!=nil{return nil,err};out=append(out,b)};return out,rows.Err()
}
func(s *Service) DeleteBackup(id int64)error{if id<=0{return fmt.Errorf("backup id must be positive")};res,err:=s.db.Conn.Exec("DELETE FROM backups WHERE id=?",id);if err!=nil{return err};n,err:=res.RowsAffected();if err!=nil{return err};if n==0{return sql.ErrNoRows};return nil}
func(s *Service) PruneBackups(deviceID int64,keep int)(int64,error){
 if deviceID<=0||keep<1||keep>1000{return 0,fmt.Errorf("device id and keep count must be within safe bounds")}
 res,err:=s.db.Conn.Exec("DELETE FROM backups WHERE device_id=? AND id NOT IN (SELECT id FROM backups WHERE device_id=? ORDER BY id DESC LIMIT ?)",deviceID,deviceID,keep);if err!=nil{return 0,err};return res.RowsAffected()
}
