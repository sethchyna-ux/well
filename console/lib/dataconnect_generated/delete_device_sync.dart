part of 'generated.dart';

class DeleteDeviceSyncVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  DeleteDeviceSyncVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<DeleteDeviceSyncData> dataDeserializer = (dynamic json)  => DeleteDeviceSyncData.fromJson(jsonDecode(json));
  Serializer<DeleteDeviceSyncVariables> varsSerializer = (DeleteDeviceSyncVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<DeleteDeviceSyncData, DeleteDeviceSyncVariables>> execute() {
    return ref().execute();
  }

  MutationRef<DeleteDeviceSyncData, DeleteDeviceSyncVariables> ref() {
    DeleteDeviceSyncVariables vars= DeleteDeviceSyncVariables(id: id,);
    return _dataConnect.mutation("DeleteDeviceSync", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class DeleteDeviceSyncDeviceSyncDelete {
  final String id;
  DeleteDeviceSyncDeviceSyncDelete.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteDeviceSyncDeviceSyncDelete otherTyped = other as DeleteDeviceSyncDeviceSyncDelete;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteDeviceSyncDeviceSyncDelete({
    required this.id,
  });
}

@immutable
class DeleteDeviceSyncData {
  final DeleteDeviceSyncDeviceSyncDelete? deviceSync_delete;
  DeleteDeviceSyncData.fromJson(dynamic json):
  
  deviceSync_delete = json['deviceSync_delete'] == null ? null : DeleteDeviceSyncDeviceSyncDelete.fromJson(json['deviceSync_delete']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteDeviceSyncData otherTyped = other as DeleteDeviceSyncData;
    return deviceSync_delete == otherTyped.deviceSync_delete;
    
  }
  @override
  int get hashCode => deviceSync_delete.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (deviceSync_delete != null) {
      json['deviceSync_delete'] = deviceSync_delete!.toJson();
    }
    return json;
  }

  DeleteDeviceSyncData({
    this.deviceSync_delete,
  });
}

@immutable
class DeleteDeviceSyncVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  DeleteDeviceSyncVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteDeviceSyncVariables otherTyped = other as DeleteDeviceSyncVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteDeviceSyncVariables({
    required this.id,
  });
}

