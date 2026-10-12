CREATE TABLE Hotel(hotelNo TEXT PRIMARY KEY,name TEXT NOT NULL,city TEXT NOT NULL);
CREATE TABLE Room(roomNo TEXT,hotelNo TEXT REFERENCES Hotel(hotelNo),type TEXT,price REAL CHECK(price>=0),PRIMARY KEY(hotelNo,roomNo));
CREATE TABLE Guest(guestNo TEXT PRIMARY KEY,name TEXT NOT NULL,guestCity TEXT);
CREATE TABLE Booking(hotelNo TEXT,roomNo TEXT,guestNo TEXT REFERENCES Guest(guestNo),dateFrom TEXT,dateTo TEXT,CHECK(dateTo>dateFrom),PRIMARY KEY(hotelNo,roomNo,dateFrom),FOREIGN KEY(hotelNo,roomNo) REFERENCES Room(hotelNo,roomNo));
INSERT INTO Hotel VALUES('H1','华天','长沙'),('H2','湖畔','岳阳');
INSERT INTO Room VALUES('101','H1','单人',200),('102','H1','双人',350),('101','H2','双人',280);
INSERT INTO Guest VALUES('G1','张三','长沙'),('G2','李四','北京'),('G3','王五','武汉');
INSERT INTO Booking VALUES('H1','101','G1','2026-09-01','2026-09-03'),('H1','102','G2','2026-09-02','2026-09-04');
