package service

import (
 "fmt"
 "strings"
 "time"
)

// Setting returns a key's configured value without exposing credentials.
func(s *Service) Setting(key string)(string,error){if strings.TrimSpace(key)==""{return "",fmt.Errorf("setting key required")};var value string;err:=s.db.Conn.QueryRow("SELECT value FROM settings WHERE key=?",key).Scan(&value);return value,err}
func(s *Service) SetSetting(key,value string)error{
 key=strings.TrimSpace(key);if key==""||len(key)>120||len(value)>4096||strings.ContainsRune(key,0){return fmt.Errorf("invalid setting")}
 _,err:=s.db.Conn.Exec("INSERT INTO settings(key,value,updated_at) VALUES(?,?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at",key,value,time.Now().UTC().Format(time.RFC3339Nano));return err
}
func(s *Service) DeleteSetting(key string)error{if strings.TrimSpace(key)==""{return fmt.Errorf("setting key required")};_,err:=s.db.Conn.Exec("DELETE FROM settings WHERE key=?",key);return err}
func(s *Service) Settings() (map[string]string,error){rows,err:=s.db.Conn.Query("SELECT key,value FROM settings WHERE key NOT LIKE '%secret%' AND key NOT LIKE '%password%' ORDER BY key");if err!=nil{return nil,err};defer rows.Close();out:=map[string]string{};for rows.Next(){var k,v string;if err:=rows.Scan(&k,&v);err!=nil{return nil,err};out[k]=v};return out,rows.Err()}
func(s *Service) AddJobWithValidation(name,description,command string)(int64,error){
 if strings.TrimSpace(name)==""||len(name)>160{return 0,fmt.Errorf("job name is required and must be at most 160 characters")}
 if len(description)>2000{return 0,fmt.Errorf("description exceeds 2000 characters")}
 if strings.TrimSpace(command)==""||len(command)>8192{return 0,fmt.Errorf("command template is required and must be at most 8192 characters")}
 if strings.ContainsRune(command,0){return 0,fmt.Errorf("command template contains NUL")}
 return s.AddJob(strings.TrimSpace(name),strings.TrimSpace(description),command)
}
