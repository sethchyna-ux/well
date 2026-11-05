part of 'generated.dart';

class DeleteShellConfigVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  DeleteShellConfigVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<DeleteShellConfigData> dataDeserializer = (dynamic json)  => DeleteShellConfigData.fromJson(jsonDecode(json));
  Serializer<DeleteShellConfigVariables> varsSerializer = (DeleteShellConfigVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<DeleteShellConfigData, DeleteShellConfigVariables>> execute() {
    return ref().execute();
  }

  MutationRef<DeleteShellConfigData, DeleteShellConfigVariables> ref() {
    DeleteShellConfigVariables vars= DeleteShellConfigVariables(id: id,);
    return _dataConnect.mutation("DeleteShellConfig", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class DeleteShellConfigShellConfigDelete {
  final String id;
  DeleteShellConfigShellConfigDelete.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteShellConfigShellConfigDelete otherTyped = other as DeleteShellConfigShellConfigDelete;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteShellConfigShellConfigDelete({
    required this.id,
  });
}

@immutable
class DeleteShellConfigData {
  final DeleteShellConfigShellConfigDelete? shellConfig_delete;
  DeleteShellConfigData.fromJson(dynamic json):
  
  shellConfig_delete = json['shellConfig_delete'] == null ? null : DeleteShellConfigShellConfigDelete.fromJson(json['shellConfig_delete']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteShellConfigData otherTyped = other as DeleteShellConfigData;
    return shellConfig_delete == otherTyped.shellConfig_delete;
    
  }
  @override
  int get hashCode => shellConfig_delete.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (shellConfig_delete != null) {
      json['shellConfig_delete'] = shellConfig_delete!.toJson();
    }
    return json;
  }

  DeleteShellConfigData({
    this.shellConfig_delete,
  });
}

@immutable
class DeleteShellConfigVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  DeleteShellConfigVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteShellConfigVariables otherTyped = other as DeleteShellConfigVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteShellConfigVariables({
    required this.id,
  });
}

