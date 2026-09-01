part of 'generated.dart';

class SyncDeviceVariablesBuilder {
  String name;

  final FirebaseDataConnect _dataConnect;
  SyncDeviceVariablesBuilder(this._dataConnect, {required  this.name,});
  Deserializer<SyncDeviceData> dataDeserializer = (dynamic json)  => SyncDeviceData.fromJson(jsonDecode(json));
  Serializer<SyncDeviceVariables> varsSerializer = (SyncDeviceVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<SyncDeviceData, SyncDeviceVariables>> execute() {
    return ref().execute();
  }

  MutationRef<SyncDeviceData, SyncDeviceVariables> ref() {
    SyncDeviceVariables vars= SyncDeviceVariables(name: name,);
    return _dataConnect.mutation("SyncDevice", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class SyncDeviceDeviceSyncInsert {
  final String id;
  SyncDeviceDeviceSyncInsert.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final SyncDeviceDeviceSyncInsert otherTyped = other as SyncDeviceDeviceSyncInsert;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  SyncDeviceDeviceSyncInsert({
    required this.id,
  });
}

@immutable
class SyncDeviceData {
  final SyncDeviceDeviceSyncInsert deviceSync_insert;
  SyncDeviceData.fromJson(dynamic json):
  
  deviceSync_insert = SyncDeviceDeviceSyncInsert.fromJson(json['deviceSync_insert']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final SyncDeviceData otherTyped = other as SyncDeviceData;
    return deviceSync_insert == otherTyped.deviceSync_insert;
    
  }
  @override
  int get hashCode => deviceSync_insert.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['deviceSync_insert'] = deviceSync_insert.toJson();
    return json;
  }

  SyncDeviceData({
    required this.deviceSync_insert,
  });
}

@immutable
class SyncDeviceVariables {
  final String name;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  SyncDeviceVariables.fromJson(Map<String, dynamic> json):
  
  name = nativeFromJson<String>(json['name']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final SyncDeviceVariables otherTyped = other as SyncDeviceVariables;
    return name == otherTyped.name;
    
  }
  @override
  int get hashCode => name.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['name'] = nativeToJson<String>(name);
    return json;
  }

  SyncDeviceVariables({
    required this.name,
  });
}

