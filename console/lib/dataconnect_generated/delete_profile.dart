part of 'generated.dart';

class DeleteProfileVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  DeleteProfileVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<DeleteProfileData> dataDeserializer = (dynamic json)  => DeleteProfileData.fromJson(jsonDecode(json));
  Serializer<DeleteProfileVariables> varsSerializer = (DeleteProfileVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<DeleteProfileData, DeleteProfileVariables>> execute() {
    return ref().execute();
  }

  MutationRef<DeleteProfileData, DeleteProfileVariables> ref() {
    DeleteProfileVariables vars= DeleteProfileVariables(id: id,);
    return _dataConnect.mutation("DeleteProfile", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class DeleteProfileProfileDelete {
  final String id;
  DeleteProfileProfileDelete.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteProfileProfileDelete otherTyped = other as DeleteProfileProfileDelete;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteProfileProfileDelete({
    required this.id,
  });
}

@immutable
class DeleteProfileData {
  final DeleteProfileProfileDelete? profile_delete;
  DeleteProfileData.fromJson(dynamic json):
  
  profile_delete = json['profile_delete'] == null ? null : DeleteProfileProfileDelete.fromJson(json['profile_delete']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteProfileData otherTyped = other as DeleteProfileData;
    return profile_delete == otherTyped.profile_delete;
    
  }
  @override
  int get hashCode => profile_delete.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (profile_delete != null) {
      json['profile_delete'] = profile_delete!.toJson();
    }
    return json;
  }

  DeleteProfileData({
    this.profile_delete,
  });
}

@immutable
class DeleteProfileVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  DeleteProfileVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteProfileVariables otherTyped = other as DeleteProfileVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteProfileVariables({
    required this.id,
  });
}

