part of 'generated.dart';

class GetDeviceSyncVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  GetDeviceSyncVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<GetDeviceSyncData> dataDeserializer = (dynamic json)  => GetDeviceSyncData.fromJson(jsonDecode(json));
  Serializer<GetDeviceSyncVariables> varsSerializer = (GetDeviceSyncVariables vars) => jsonEncode(vars.toJson());
  Future<QueryResult<GetDeviceSyncData, GetDeviceSyncVariables>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<GetDeviceSyncData, GetDeviceSyncVariables> ref() {
    GetDeviceSyncVariables vars= GetDeviceSyncVariables(id: id,);
    return _dataConnect.query("GetDeviceSync", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class GetDeviceSyncDeviceSync {
  final String deviceName;
  final Timestamp lastSeen;
  GetDeviceSyncDeviceSync.fromJson(dynamic json):
  
  deviceName = nativeFromJson<String>(json['deviceName']),
  lastSeen = Timestamp.fromJson(json['lastSeen']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetDeviceSyncDeviceSync otherTyped = other as GetDeviceSyncDeviceSync;
    return deviceName == otherTyped.deviceName && 
    lastSeen == otherTyped.lastSeen;
    
  }
  @override
  int get hashCode => Object.hashAll([deviceName.hashCode, lastSeen.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['deviceName'] = nativeToJson<String>(deviceName);
    json['lastSeen'] = lastSeen.toJson();
    return json;
  }

  GetDeviceSyncDeviceSync({
    required this.deviceName,
    required this.lastSeen,
  });
}

@immutable
class GetDeviceSyncData {
  final GetDeviceSyncDeviceSync? deviceSync;
  GetDeviceSyncData.fromJson(dynamic json):
  
  deviceSync = json['deviceSync'] == null ? null : GetDeviceSyncDeviceSync.fromJson(json['deviceSync']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetDeviceSyncData otherTyped = other as GetDeviceSyncData;
    return deviceSync == otherTyped.deviceSync;
    
  }
  @override
  int get hashCode => deviceSync.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (deviceSync != null) {
      json['deviceSync'] = deviceSync!.toJson();
    }
    return json;
  }

  GetDeviceSyncData({
    this.deviceSync,
  });
}

@immutable
class GetDeviceSyncVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  GetDeviceSyncVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetDeviceSyncVariables otherTyped = other as GetDeviceSyncVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  GetDeviceSyncVariables({
    required this.id,
  });
}

